use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::GitIdentity;

use crate::support::sandbox;

use super::mismatch;

/// `Sandbox内のGit` identityがmetadataと一致するかを変更せずに観測する。
pub fn observe(host: &dyn HostEnvironment, sandbox: &str, git: &GitIdentity) -> Result<bool> {
    let name = observe_setting(host, sandbox, "user.name", &git.user_name)?;
    let email = observe_setting(host, sandbox, "user.email", &git.user_email)?;
    Ok(name && email)
}

fn observe_setting(
    host: &dyn HostEnvironment,
    sandbox: &str,
    key: &str,
    expected: &str,
) -> Result<bool> {
    let outcome = sandbox::exec(host, sandbox, &["git", "config", "--global", "--get", key])?;
    if !outcome.success() {
        return Ok(false);
    }
    let observed = outcome.stdout_text().trim().to_string();
    if observed == expected {
        return Ok(true);
    }
    if observed.is_empty() {
        return Ok(false);
    }
    Err(mismatch(sandbox, key, &observed, expected))
}
