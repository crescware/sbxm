//! Git identity。
//!
//! 案件の名義は利用者が選ぶ。選ばれた値はglobal configへ既定として保存され、登録時に
//! 案件metadataへsnapshotされる。Sandbox内へ設定するのは、そのsnapshotだけである。
//!
//! hostの`git config --global`はこの選択の候補にしかならない。読めなくても案件を
//! 止めない。値を決めるのはhostではなく利用者である。
//!
//! Sandbox内の既存の設定値が同じならそのままにし、異なる場合は別の利用者のSandboxで
//! ある可能性があるため自動で上書きしない。
//!
//! `gh`のprotocol設定もここに置く。値の性質が同じで、一致しない値の扱いを同じ形で
//! 決めるためである。呼ぶかどうかは`tools`が`gh`の有無で決める。

use crate::command::{CommandSpec, HostEnvironment, TimeoutClass};
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{GitIdentity, validate_git_identity_value};
use crate::msg;

use super::sandbox;

/// 入力欄へ置くhostの候補を読む。
///
/// 候補であって決定ではないため、読めない場合を失敗として扱わない。不在、空、複数値、
/// 観測不能のいずれも空文字とし、利用者が自分で打てる空欄として現れる。
///
/// `--get-all`は複数回宣言された値をすべて返す。1つに絞れない設定から候補を選ばない。
pub fn candidate_from_host(host: &dyn HostEnvironment, key: &str) -> String {
    let spec = CommandSpec::probe("git", &["config", "--global", "--get-all", key])
        .timeout(TimeoutClass::LocalFilesystem);
    let Ok(outcome) = host.run(&spec) else {
        return String::new();
    };
    if !outcome.success() {
        return String::new();
    }
    // 空の宣言も1つの宣言である。落として1件に見せると、gitが解決する値と食い違う。
    let stdout = outcome.stdout_text();
    let values: Vec<&str> = stdout.lines().collect();
    let [value] = values.as_slice() else {
        return String::new();
    };
    let value = value.trim();
    if validate_git_identity_value(value).is_err() {
        return String::new();
    }
    value.to_string()
}

/// GitHubとのやり取りに使うprotocol。Sandbox内のcloneはHTTPSを使う。
const GIT_PROTOCOL: &str = "https";
const GITHUB_HOST: &str = "github.com";

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

/// Sandbox内の`gh``が使うprotocolがHTTPSであることを確かめる`。
///
/// sbxmはこの値を読まない。remote URLは自分でHTTPSとして書く。`gh`自身の既定も
/// `https`であり、設定fileを持たないSandboxでは一致を観測して終わる。書き込みへ
/// 進むのは`gh`が答えなかった場合だけである。
///
/// 実際に効くのは、中で`ssh`へ変えられていた場合である。SandboxにSSH鍵は無いため、
/// その`gh``はGitHubへ到達できない`。
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
