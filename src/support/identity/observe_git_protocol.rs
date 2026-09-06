use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::{GIT_PROTOCOL, GITHUB_HOST, mismatch};

/// `gh`の`GitHub` `protocol`設定がHTTPSかを変更せずに観測する。
pub fn observe_git_protocol(host: &dyn HostEnvironment, sandbox: &str) -> Result<bool> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["gh", "config", "get", "git_protocol", "--host", GITHUB_HOST],
    )?;
    if !outcome.success() {
        return Ok(false);
    }
    let observed = outcome.stdout_text().trim().to_string();
    if observed == GIT_PROTOCOL {
        return Ok(true);
    }
    if observed.is_empty() {
        return Ok(false);
    }
    Err(mismatch(
        sandbox,
        "gh git_protocol",
        &observed,
        GIT_PROTOCOL,
    ))
}
