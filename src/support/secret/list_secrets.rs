use crate::boundary::host::protocol::{SecretListing, parse_secret_listing};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

/// 全scopeのsecretの登録を読む。
///
/// scopeで絞らずに読む。global scopeのservice secretでもSandboxはsentinelの差し替えを
/// 受けられるため、両方を見てから判定する。値は`--json`でも伏せられて返る。
pub(super) fn list_secrets(host: &dyn HostEnvironment) -> Result<SecretListing> {
    let spec = CommandSpec::capture("sbx", &["secret", "ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    parse_secret_listing(&outcome.stdout_text())
}
