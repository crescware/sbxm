use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::GITHUB_HOST;

/// `gh`の`GitHub` `protocol`設定が設定済みかを、変更せずに観測する。
///
/// HTTPS以外の値も、設定済みとして答える。Sandboxの中で利用者が選んだ値であり、
/// 欠けているものではない。
pub fn observe_git_protocol(host: &dyn HostEnvironment, sandbox: &str) -> Result<bool> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["gh", "config", "get", "git_protocol", "--host", GITHUB_HOST],
    )?;
    if !outcome.success() {
        return Ok(false);
    }
    Ok(!outcome.stdout_text().trim().is_empty())
}
