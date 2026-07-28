//! Sandbox内のGit identity。
//!
//! 既存の設定値が同じならそのままにし、異なる場合は別の利用者のSandboxである
//! 可能性があるため自動で上書きしない。
//!
//! `gh`のprotocol設定もここに置く。値の性質が同じで、一致しない値の扱いを同じ形で
//! 決めるためである。呼ぶかどうかは`tools`が`gh`の有無で決める。

use crate::command::HostEnvironment;
use crate::config::GitIdentity;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::sandbox;

/// GitHubとのやり取りに使うprotocol。Sandbox内のcloneはHTTPSを使う。
const GIT_PROTOCOL: &str = "https";
const GITHUB_HOST: &str = "github.com";

/// Git identityを設定する。
///
/// `gh`はsbxm自身のworkflowが一度も呼ばない。Sandbox内のcloneもfetchも素のgitがHTTPSで
/// 行い、認証はcredential helperが担う。そのため`gh`の設定はここには含めない。
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

/// Sandbox内の`gh`が使うprotocolがHTTPSであることを確かめる。
///
/// sbxmはこの値を読まない。remote URLは自分でHTTPSとして書く。`gh`自身の既定も
/// `https`であり、設定fileを持たないSandboxでは一致を観測して終わる。書き込みへ
/// 進むのは`gh`が答えなかった場合だけである。
///
/// 実際に効くのは、中で`ssh`へ変えられていた場合である。SandboxにSSH鍵は無いため、
/// その`gh`はGitHubへ到達できない。
pub(super) fn ensure_git_protocol(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["gh", "config", "get", "git_protocol", "--host", GITHUB_HOST],
    )?;
    if outcome.success() {
        let observed = outcome.stdout_text().trim().to_string();
        if observed == GIT_PROTOCOL {
            return Ok(());
        }
        if !observed.is_empty() {
            return Err(mismatch(
                sandbox,
                "gh git_protocol",
                &observed,
                GIT_PROTOCOL,
            ));
        }
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

fn mismatch(sandbox: &str, key: &str, observed: &str, expected: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxIdentityMismatch,
            msg!(
                "error-sandbox-identity-mismatch",
                sandbox = sandbox,
                key = key,
                observed = observed,
                expected = expected
            ),
        )
        .remediation(msg!("remediation-sandbox-identity-mismatch")),
    )
}

#[cfg(test)]
#[path = "identity_test.rs"]
mod identity_test;
