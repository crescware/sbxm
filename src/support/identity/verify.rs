use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::GitIdentity;

use crate::support::sandbox;

use super::mismatch;

/// 既存の `Sandbox` にある `Git` identity を read-only で確認する。
pub(crate) fn verify(host: &dyn HostEnvironment, sandbox: &str, git: &GitIdentity) -> Result<()> {
    verify_value(host, sandbox, "user.name", &git.user_name)?;
    verify_value(host, sandbox, "user.email", &git.user_email)
}

fn verify_value(
    host: &dyn HostEnvironment,
    sandbox: &str,
    key: &str,
    expected: &str,
) -> Result<()> {
    let outcome = sandbox::exec(host, sandbox, &["git", "config", "--global", "--get", key])?;
    if outcome.success() {
        let observed = outcome.stdout_text().trim().to_string();
        if !observed.is_empty() && observed != expected {
            return Err(mismatch(sandbox, key, &observed, expected));
        }
    }
    Ok(())
}
