//! Git identity。
//!
//! 新規登録時にhostの`git config --global`から取得し、案件metadataへsnapshotする。
//! Sandbox内へ設定するのは、そのsnapshotだけである。
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
use crate::ui::Remediation;

use super::sandbox;

/// hostが宣言しているGit identityを読む。
///
/// 不在、空、複数値、観測不能のいずれも、推測で補わず拒否する。案件を作る前に呼び、
/// 設定するcommandを案内する。
pub fn from_host(host: &dyn HostEnvironment) -> Result<GitIdentity> {
    Ok(GitIdentity {
        user_name: read_global(host, "user.name")?,
        user_email: read_global(host, "user.email")?,
    })
}

fn read_global(host: &dyn HostEnvironment, key: &str) -> Result<String> {
    // `--get-all`は複数回宣言された値をすべて返す。1つに絞れない設定を黙って選ばない。
    let spec = CommandSpec::probe("git", &["config", "--global", "--get-all", key])
        .timeout(TimeoutClass::LocalFilesystem);
    let outcome = host.run(&spec)?;
    if !outcome.success() {
        return Err(unavailable(key, "no value is set"));
    }
    let stdout = outcome.stdout_text();
    let values: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let [value] = values.as_slice() else {
        let detail = format!("{} values are set", values.len());
        return Err(unavailable(key, &detail));
    };
    validate_git_identity_value(value).map_err(|detail| unavailable(key, detail))?;
    Ok((*value).to_string())
}

/// hostのGit identityを、設定するcommandとともに要求する。
fn unavailable(key: &str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::GitIdentityUnavailable,
            msg!("error-git-identity-unavailable", key = key, detail = detail),
        )
        .remediation(
            Remediation::text(msg!("remediation-git-identity-unavailable"))
                .try_run(format!("git config --global {key} <value>")),
        ),
    )
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
