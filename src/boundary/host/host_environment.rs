use std::io::Write;
use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, PtyConfirmedCommand, TerminalCommand, output_too_large};

/// hostに対する外部commandの実行。testでは差し替える。
pub trait HostEnvironment {
    fn command_exists(&self, program: &str) -> bool;

    /// 出力をcaptureして実行する。
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome>;

    /// 出力が端末まで届くcommandを実行する。
    ///
    /// 既定は`run`へ委ね、captureできた出力をまとめて渡す。実際のhostだけが、実行中の
    /// byteを届いた順に流す。testのhostは、何が端末まで届いたかをこの既定で観測できる。
    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome> {
        let outcome = self.run(command.spec())?;
        output.relay(&outcome.stdout);
        output.relay(&outcome.stderr);
        output.finished();
        Ok(outcome)
    }

    /// 端末を引き渡したcommandを実行し、終わるのを待つあいだ`every`ごとに`tick`を呼ぶ。
    ///
    /// `tick`は端末へ何も書かない手続きとする。既定は`run_with_terminal`で実行したあとに
    /// `tick`を1回だけ呼ぶ。実際のhostだけが、実行中に時間を測って呼ぶ。testのhostは、
    /// 実行中の手続きが走ることをこの既定で観測できる。
    fn run_with_terminal_ticking(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
        _every: Duration,
        tick: &mut dyn FnMut(),
    ) -> Result<CommandOutcome> {
        let outcome = self.run_with_terminal(command, output)?;
        tick();
        Ok(outcome)
    }

    /// 出力をcaptureして実行し、stdoutだけを`sink`へ流す。`limit`byteを超えたら拒否する。
    ///
    /// 返す結果のstdoutは空である。既定は`run`へ委ね、captureできたstdoutをまとめて渡す。
    /// 実際のhostだけが、届いた順に流し、上限を超えた時点で子を終わらせる。
    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome> {
        let mut outcome = self.run(spec)?;
        if u64::try_from(outcome.stdout.len()).unwrap_or(u64::MAX) > limit {
            return Err(output_too_large(spec, limit));
        }
        sink.write_all(&outcome.stdout)
            .map_err(|error| super::unreadable(spec, &error.to_string()))?;
        outcome.stdout.clear();
        Ok(outcome)
    }

    /// 確認promptにだけ答える、PTYの上でのcommand実行。利用者へ何も中継しない。
    ///
    /// 既定は`run`へ委ね、答えたものとして進める。本物のPTYで期待するpromptを確認
    /// してから答えるのは、実際のhostだけである。
    fn run_pty_confirmed(&self, command: &PtyConfirmedCommand) -> Result<CommandOutcome> {
        self.run(&command.as_capture_spec())
    }
}
