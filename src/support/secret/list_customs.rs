use crate::boundary::host::protocol::{CustomSecret, parse_custom_secrets};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

/// 登録済みのcustom secretを読む。
///
/// scopeを指定せずに読む。global scopeの登録もSandboxへ効くため、両方を見てから
/// 判定する。scopeを引数で絞ると、どちらか一方しか見えない。
///
/// `--service`で絞ると出力へ値の一部が現れる。一覧のまま読む形で呼ぶ。
pub(super) fn list_customs(host: &dyn HostEnvironment) -> Result<Vec<CustomSecret>> {
    let spec = CommandSpec::capture("sbx", &["secret", "ls"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    let outcome = host.run(&spec)?.require_success()?;
    parse_custom_secrets(&outcome.stdout_text())
}
