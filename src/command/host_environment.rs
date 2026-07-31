use crate::diagnostics::Result;

use super::{CommandOutcome, CommandSpec};

/// hostに対する外部commandの実行。testでは差し替える。
pub trait HostEnvironment {
    fn command_exists(&self, program: &str) -> bool;
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome>;
}
