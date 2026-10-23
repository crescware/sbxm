use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::{GIT_PROTOCOL, GITHUB_HOST};

/// Sandbox内の`gh`が使うprotocolが未設定なら、HTTPSにする。
///
/// sbxmはこの値を読まない。remote URLは自分でHTTPSとして書く。`gh`自身の既定も
/// `https`であり、設定fileを持たないSandboxでは一致を観測して終わる。書き込みへ
/// 進むのは`gh`が答えなかった場合だけである。
///
/// 既に値があれば、HTTPSでなくても利用者が選んだ値として残し、上書きも拒否もしない。
pub fn ensure_git_protocol(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["gh", "config", "get", "git_protocol", "--host", GITHUB_HOST],
    )?;
    if outcome.success() && !outcome.stdout_text().trim().is_empty() {
        return Ok(());
    }

    sandbox::exec(
        host,
        sandbox,
        &[
            "gh",
            "config",
            "set",
            "git_protocol",
            GIT_PROTOCOL,
            "--host",
            GITHUB_HOST,
        ],
    )?
    .require_success()?;
    Ok(())
}
