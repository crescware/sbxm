use std::io::Write;
use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{
    CommandOutcome, CommandSpec, HostEnvironment, PtyConfirmedCommand, TerminalCommand,
    exists_on_path, run, run_pty_confirmed, run_streaming, run_terminal_ticking, run_with_terminal,
};

/// 実際のhost。
pub struct RealHost;

impl HostEnvironment for RealHost {
    fn command_exists(&self, program: &str) -> bool {
        exists_on_path(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        run(spec)
    }

    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome> {
        run_with_terminal(command, output)
    }

    fn run_with_terminal_ticking(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
        every: Duration,
        tick: &mut dyn FnMut(),
    ) -> Result<CommandOutcome> {
        run_terminal_ticking(command, output, every, tick)
    }

    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome> {
        run_streaming(spec, sink, limit)
    }

    fn run_pty_confirmed(&self, command: &PtyConfirmedCommand) -> Result<CommandOutcome> {
        run_pty_confirmed(command)
    }
}
