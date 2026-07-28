//! Git参照の検証と表記。
//!
//! 利用者が指定するのはremote branch名だけであり、sbxmが組み立てるのは
//! `refs/remotes/origin/<branch>`という完全なremote-tracking refである。

use crate::error::{ErrorId, Result, fail};
use crate::msg;

/// remote-tracking refの接頭辞。
const ORIGIN_REF_PREFIX: &str = "refs/remotes/origin/";

/// branch名の上限。
const MAX_BRANCH_BYTES: usize = 255;

/// remote branch名として受け付けるかを判定する。
///
/// Sandbox内では`git check-ref-format --branch`が同じ値を再検証する。ここでは、
/// 外部commandへ渡す前に確実に拒否できる条件だけを見る。
pub fn validate_branch_name(value: &str) -> Result<()> {
    let invalid = |detail: &'static str| {
        fail(
            ErrorId::InvalidBranchName,
            msg!("error-invalid-branch-name", value = value, detail = detail),
        )
    };

    if value.is_empty() {
        return invalid("the branch name is empty");
    }
    if value.len() > MAX_BRANCH_BYTES {
        return invalid("the branch name is longer than 255 bytes");
    }
    if value.contains('\0') {
        return invalid("the branch name contains a NUL byte");
    }
    if value.contains('\n') || value.contains('\r') {
        return invalid("the branch name contains a line break");
    }
    if value.starts_with('-') {
        // 先頭が`-`の値は外部commandのoptionとして解釈され得る。
        return invalid("the branch name starts with a hyphen");
    }
    Ok(())
}

/// `refs/remotes/origin/<branch>`
pub fn origin_ref(branch: &str) -> String {
    format!("{ORIGIN_REF_PREFIX}{branch}")
}

/// 対応するhosting service。MVPはGitHubだけを対象とする。
const GITHUB_HOST: &str = "github.com";

/// host cloneが使うSSH remote。
pub fn ssh_remote_url(owner: &str, repository: &str) -> String {
    format!("git@{GITHUB_HOST}:{owner}/{repository}.git")
}

/// Sandbox内のcloneが使うHTTPS remote。
pub fn https_remote_url(owner: &str, repository: &str) -> String {
    format!("https://{GITHUB_HOST}/{owner}/{repository}.git")
}

/// remote URLを比較用のcanonical project IDへ正規化する。
///
/// 同じrepositoryを指すSSHとHTTPSの表記を同じ値へ寄せる。GitHub以外を指すURLと、
/// `<owner>/<repository>`を読み取れないURLは`None`とする。
pub fn canonical_id_of_remote(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = if let Some(rest) = url.strip_prefix("git@") {
        // git@github.com:owner/repository.git
        let (host, path) = rest.split_once(':')?;
        require_github(host)?;
        path
    } else {
        let rest = url
            .strip_prefix("ssh://git@")
            .or_else(|| url.strip_prefix("ssh://"))
            .or_else(|| url.strip_prefix("https://"))
            .or_else(|| url.strip_prefix("http://"))
            .or_else(|| url.strip_prefix("git://"))?;
        let (authority, path) = rest.split_once('/')?;
        // ssh://git@github.com:22/owner/repository.git
        let host = authority.rsplit('@').next()?;
        let host = host.split(':').next()?;
        require_github(host)?;
        path
    };

    let path = rest.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repository) = path.split_once('/')?;
    if owner.is_empty() || repository.is_empty() || repository.contains('/') {
        return None;
    }
    Some(format!(
        "{}/{}",
        owner.to_ascii_lowercase(),
        repository.to_ascii_lowercase()
    ))
}

fn require_github(host: &str) -> Option<()> {
    host.eq_ignore_ascii_case(GITHUB_HOST).then_some(())
}

#[cfg(test)]
#[path = "git_test.rs"]
mod git_test;
