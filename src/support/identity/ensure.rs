use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::GitIdentity;

use crate::support::sandbox;

/// Git identityを設定する。
///
/// Sandbox内のcloneもfetchも素のgitがHTTPSで行い、認証はcredential helperが担う。
///
/// 書くのは未設定のkeyだけである。既に値があれば、記録と違っても利用者が選んだ値として
/// 残し、上書きも拒否もしない。
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
    if outcome.success() && !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }

    sandbox::exec(host, sandbox, &["git", "config", "--global", key, expected])?
        .require_success()?;
    Ok(())
}
