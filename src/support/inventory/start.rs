use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;

use crate::design::ProgressSink;

/// 非対話でSandboxを起動する。
pub fn start(
    host: &dyn HostEnvironment,
    sandbox: &str,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-starting-sandbox"));
    let spec = CommandSpec::passthrough("sbx", &["exec", sandbox, "--", "/bin/true"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;
    Ok(())
}
