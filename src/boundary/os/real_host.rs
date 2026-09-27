use std::io::Write;
use std::time::Duration;

use crate::boundary::host::{
    CommandOutcome, CommandSpec, HostEnvironment, PtyConfirmedCommand, TerminalCommand,
    exists_on_path, run_inner, run_pty_confirmed, run_streaming, run_terminal_inner,
};
use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::SystemClock;

/// 実際のhost。
///
/// このdirectoryの`system_*.rs`が実装するOSの基本操作と`SystemClock`を、判断のcodeへ
/// そのまま渡す。何をどの順で呼ぶかは、すべて`boundary::host`が決める。
pub struct RealHost;

impl HostEnvironment for RealHost {
    fn command_exists(&self, program: &str) -> bool {
        exists_on_path(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        run_inner(self, &SystemClock, spec, spec.timeout.duration())
    }

    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome> {
        run_terminal_inner(self, &SystemClock, command, output, None)
    }

    fn run_with_terminal_ticking(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
        every: Duration,
        tick: &mut dyn FnMut(),
    ) -> Result<CommandOutcome> {
        run_terminal_inner(self, &SystemClock, command, output, Some((every, tick)))
    }

    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome> {
        run_streaming(self, &SystemClock, spec, sink, limit)
    }

    fn run_pty_confirmed(&self, command: &PtyConfirmedCommand) -> Result<CommandOutcome> {
        run_pty_confirmed(self, &SystemClock, command)
    }
}

#[cfg(test)]
#[path = "real_host_test.rs"]
mod real_host_test;
