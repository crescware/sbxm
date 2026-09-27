use std::io::Write;
use std::time::Duration;

use crate::boundary::host::{
    CommandOutcome, CommandSpec, HostEnvironment, PtyConfirmedCommand, TerminalCommand,
    run_pty_confirmed, run_streaming, run_terminal_inner,
};
use crate::design::ExternalOutput;
use crate::diagnostics::Result;
use crate::testing::command::ScriptedOs;

/// 実行全体への答えだけを持つtestのhost。fakeはこれを実装する。
///
/// 端末を引き渡す実行、出力を流す実行、PTYの上の実行は、答えを`ScriptedOs`で1歩ずつの基本操作に
/// 直し、実物と同じ判断のcodeへ渡す。tickの予定、出力の上限、promptの確認は、fakeでも実物と同じ
/// codeが決める。
pub trait AnsweredHost {
    fn has_command(&self, program: &str) -> bool;

    /// 1回の実行への答え。
    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome>;

    /// 端末を引き渡したsessionが終わるまでに経つ時間。既定は0で、途中の手続きは呼ばれない。
    fn session_length(&self, _command: &TerminalCommand, _every: Duration) -> Duration {
        Duration::ZERO
    }
}

impl<T: AnsweredHost> HostEnvironment for T {
    fn command_exists(&self, program: &str) -> bool {
        self.has_command(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.answer(spec)
    }

    /// captureできた出力をまとめて渡す（今の既定と同じ）。何が端末まで届いたかを、testは
    /// これで観測する。
    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome> {
        let outcome = self.answer(command.spec())?;
        output.relay(&outcome.stdout);
        output.relay(&outcome.stderr);
        output.finished();
        Ok(outcome)
    }

    fn run_with_terminal_ticking(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
        every: Duration,
        tick: &mut dyn FnMut(),
    ) -> Result<CommandOutcome> {
        let answer = self.answer(command.spec())?;
        let os = ScriptedOs::replaying(&answer, self.session_length(command, every));
        run_terminal_inner(&os, os.clock(), command, output, Some((every, tick)))
    }

    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome> {
        let answer = self.answer(spec)?;
        let os = ScriptedOs::replaying(&answer, Duration::ZERO);
        run_streaming(&os, os.clock(), spec, sink, limit)
    }

    fn run_pty_confirmed(&self, command: &PtyConfirmedCommand) -> Result<CommandOutcome> {
        let answer = self.answer(&command.as_capture_spec())?;
        let os = ScriptedOs::replaying_on_a_pty(&answer, command.expected_prompt());
        run_pty_confirmed(&os, os.clock(), command)
    }
}
