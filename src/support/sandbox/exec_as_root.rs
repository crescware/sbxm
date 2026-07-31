use crate::command::{CommandOutcome, HostEnvironment};
use crate::diagnostics::Result;

use super::run_exec;

/// Sandbox内でrootとしてcommandを実行する。
pub fn exec_as_root(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
) -> Result<CommandOutcome> {
    run_exec(host, sandbox, Some("root"), args)
}
