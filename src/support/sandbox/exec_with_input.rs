use crate::boundary::host::{CommandOutcome, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

use super::exec_spec;

/// Sandboxのcommandへstdinを渡す。入力を一時fileへ書かない。
pub fn exec_with_input(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    input: Vec<u8>,
) -> Result<CommandOutcome> {
    host.run(
        &exec_spec(sandbox, None, true, args, TimeoutClass::SandboxLifecycle).with_input(input),
    )
}
