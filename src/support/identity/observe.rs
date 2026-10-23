use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox;

/// `Sandbox内のGit` identityが設定済みかを、変更せずに観測する。
///
/// 記録と違う値も、設定済みとして答える。Sandboxの中で利用者が選んだ値であり、
/// 欠けているものではない。
pub fn observe(host: &dyn HostEnvironment, sandbox: &str) -> Result<bool> {
    let name = observe_setting(host, sandbox, "user.name")?;
    let email = observe_setting(host, sandbox, "user.email")?;
    Ok(name && email)
}

fn observe_setting(host: &dyn HostEnvironment, sandbox: &str, key: &str) -> Result<bool> {
    let outcome = sandbox::exec(host, sandbox, &["git", "config", "--global", "--get", key])?;
    if !outcome.success() {
        return Ok(false);
    }
    Ok(!outcome.stdout_text().trim().is_empty())
}
