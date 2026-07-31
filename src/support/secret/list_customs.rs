use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::compatibility::{CustomSecret, parse_custom_secrets};
use crate::diagnostics::Result;

/// Sandbox scopeのcustom secretを読む。
///
/// `--service`で絞ると出力へ値の一部が現れる。一覧のまま読む形で呼ぶ。
pub(super) fn list_customs(host: &dyn HostEnvironment, sandbox: &str) -> Result<Vec<CustomSecret>> {
    let spec = CommandSpec::capture("sbx", &["secret", "ls", sandbox])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    parse_custom_secrets(&outcome.stdout_text())
}
