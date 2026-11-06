use std::io::Write;
use std::time::Duration;

use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, Executable, HostEnvironment, TerminalCommand};

/// statusの外部観測で、Gitの付随する書き込みと遅延fetchを抑える。
pub(crate) struct ReadOnly<'a>(pub &'a dyn HostEnvironment);

impl HostEnvironment for ReadOnly<'_> {
    fn command_exists(&self, program: &str) -> bool {
        self.0.command_exists(program)
    }
    fn inspect_executable(&self, program: &str) -> Option<Executable> {
        self.0.inspect_executable(program)
    }
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.0.run(&spec.clone().read_only())
    }
    fn run_with_terminal(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
    ) -> Result<CommandOutcome> {
        self.0
            .run_with_terminal(&command.clone().read_only(), output)
    }
    fn run_with_terminal_ticking(
        &self,
        command: &TerminalCommand,
        output: &mut dyn ExternalOutput,
        every: Duration,
        tick: &mut dyn FnMut(),
    ) -> Result<CommandOutcome> {
        self.0
            .run_with_terminal_ticking(&command.clone().read_only(), output, every, tick)
    }
    fn run_streaming(
        &self,
        spec: &CommandSpec,
        sink: &mut dyn Write,
        limit: u64,
    ) -> Result<CommandOutcome> {
        self.0.run_streaming(&spec.clone().read_only(), sink, limit)
    }
}

#[cfg(test)]
#[path = "read_only_test.rs"]
mod read_only_test;
