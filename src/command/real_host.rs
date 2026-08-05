use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::{
    CommandOutcome, CommandSpec, HostEnvironment, TerminalCommand, exists_on_path, run,
    run_with_terminal,
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
}
