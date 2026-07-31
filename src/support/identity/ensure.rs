use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::GitIdentity;

use crate::support::sandbox;

use super::mismatch;

/// Git identityを設定する。
///
/// Sandbox内のcloneもfetchも素のgitがHTTPSで行い、認証はcredential helperが担う。
pub fn ensure(host: &dyn HostEnvironment, sandbox: &str, git: &GitIdentity) -> Result<()> {
    ensure_git_config(host, sandbox, "user.name", &git.user_name)?;
    ensure_git_config(host, sandbox, "user.email", &git.user_email)
}

fn ensure_git_config(
    host: &dyn HostEnvironment,
    sandbox: &str,
    key: &str,
    expected: &str,
) -> Result<()> {
    let outcome = sandbox::exec(host, sandbox, &["git", "config", "--global", "--get", key])?;
    if outcome.success() {
        let observed = outcome.stdout_text().trim().to_string();
        if observed == expected {
            return Ok(());
        }
        if !observed.is_empty() {
            return Err(mismatch(sandbox, key, &observed, expected));
        }
    }

    sandbox::exec(host, sandbox, &["git", "config", "--global", key, expected])?
        .require_success()?;
    Ok(())
}
