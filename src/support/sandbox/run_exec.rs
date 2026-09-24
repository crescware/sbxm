use crate::boundary::host::{CommandOutcome, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

use super::exec_spec;

pub(super) fn run_exec(
    host: &dyn HostEnvironment,
    sandbox: &str,
    user: Option<&str>,
    args: &[&str],
    timeout: TimeoutClass,
) -> Result<CommandOutcome> {
    host.run(&exec_spec(sandbox, user, false, args, timeout))
}
