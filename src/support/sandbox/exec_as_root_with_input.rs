use crate::boundary::host::{CommandOutcome, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

use super::exec_spec;

/// Sandbox内でrootとしてcommandを実行し、`input`をそのstdinへ渡す。
///
/// 通信はhostから始め、Sandboxへhostへの経路を与えない。
pub fn exec_as_root_with_input(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    input: Vec<u8>,
) -> Result<CommandOutcome> {
    let spec = exec_spec(
        sandbox,
        Some("root"),
        true,
        args,
        TimeoutClass::SandboxLifecycle,
    )
    .with_input(input);
    host.run(&spec)
}
