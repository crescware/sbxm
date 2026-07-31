use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec, HostEnvironment, exists_on_path, run};

/// 実際のhost。
pub struct RealHost;

impl HostEnvironment for RealHost {
    fn command_exists(&self, program: &str) -> bool {
        exists_on_path(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        run(spec)
    }
}
