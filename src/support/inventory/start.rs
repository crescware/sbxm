use crate::command::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;

use crate::design::ProgressSink;

use crate::support::sandbox;

/// 非対話でSandboxを起動する。
pub fn start(
    host: &dyn HostEnvironment,
    sandbox: &str,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-starting-sandbox"));
    let command = sandbox::relayed(&["exec", sandbox, "--", "/bin/true"])
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run_with_terminal(&command, progress)?
        .require_success()?;
    Ok(())
}
