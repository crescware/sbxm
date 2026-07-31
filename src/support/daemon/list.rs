use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{SandboxEntry, parse_sandbox_list};
use crate::diagnostics::Result;

/// 現在のSandbox一覧。
pub fn list(host: &dyn HostEnvironment) -> Result<Vec<SandboxEntry>> {
    let spec = CommandSpec::capture("sbx", &["ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    parse_sandbox_list(&outcome.stdout_text())
}
