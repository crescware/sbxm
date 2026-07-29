//! Docker Sandboxes daemonの観測。
//!
//! sbxmはdaemonを停止も起動もしない。daemonを止めるには動作中のSandboxを止める
//! 必要があり、作業中のSandboxを巻き込むためである。
//!
//! hostのSSH AgentがSandboxへ渡っていないことは、daemonの起動条件から推定せず、
//! 作成したSandboxの中から観測する（`sandbox::require_credentials_isolated`）。

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{SandboxEntry, parse_sandbox_list};
use crate::error::Result;

/// 現在のSandbox一覧。
pub fn list(host: &dyn HostEnvironment) -> Result<Vec<SandboxEntry>> {
    let spec = CommandSpec::capture("sbx", &["ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    parse_sandbox_list(&outcome.stdout_text())
}

#[cfg(test)]
#[path = "daemon_test.rs"]
mod daemon_test;
