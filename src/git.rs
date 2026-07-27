//! Git参照の検証と表記。
//!
//! 利用者が指定するのはremote branch名だけであり、sbxmが組み立てるのは
//! `refs/remotes/origin/<branch>`という完全なremote-tracking refである。

use crate::error::{ErrorId, Result, fail};
use crate::msg;

/// 唯一扱うremoteの名前。
pub const ORIGIN: &str = "origin";

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

/// `refs/remotes/origin/<branch>`からbranch名を取り出す。
pub fn branch_of_origin_ref(value: &str) -> Option<&str> {
    let branch = value.strip_prefix(ORIGIN_REF_PREFIX)?;
    validate_branch_name(branch).ok()?;
    Some(branch)
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
    } else if let Some(rest) = url
        .strip_prefix("ssh://git@")
        .or_else(|| url.strip_prefix("ssh://"))
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("git://"))
    {
        let (authority, path) = rest.split_once('/')?;
        // ssh://git@github.com:22/owner/repository.git
        let host = authority.rsplit('@').next()?;
        let host = host.split(':').next()?;
        require_github(host)?;
        path
    } else {
        return None;
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
mod tests {
    use super::*;

    #[test]
    fn ordinary_branch_names_are_accepted() {
        for value in ["main", "develop", "feature/login", "release-1.2", "v2"] {
            assert!(
                validate_branch_name(value).is_ok(),
                "{value} must be accepted"
            );
        }
        assert!(validate_branch_name(&"b".repeat(255)).is_ok());
    }

    #[test]
    fn branch_names_that_could_be_misread_by_an_external_command_are_refused() {
        for value in [
            "",
            "-delete",
            "with\nnewline",
            "with\0nul",
            &"b".repeat(256),
        ] {
            let error = validate_branch_name(value).expect_err("{value:?} must be refused");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::InvalidBranchName),
                "value {value:?} produced the wrong error"
            );
        }
    }

    #[test]
    fn the_same_repository_normalizes_to_one_identifier_whichever_url_form_is_used() {
        for url in [
            "git@github.com:Example-Org/Example-Repo.git",
            "git@github.com:Example-Org/Example-Repo",
            "https://github.com/Example-Org/Example-Repo.git",
            "https://github.com/example-org/example-repo/",
            "ssh://git@github.com/Example-Org/Example-Repo.git",
            "ssh://git@github.com:22/example-org/example-repo.git",
            "  https://github.com/example-org/example-repo  ",
        ] {
            assert_eq!(
                canonical_id_of_remote(url).as_deref(),
                Some("example-org/example-repo"),
                "{url} must normalize to the canonical project ID"
            );
        }
    }

    #[test]
    fn remotes_that_do_not_name_a_github_repository_are_not_normalized() {
        for url in [
            "git@gitlab.com:example-org/example-repo.git",
            "https://example.com/example-org/example-repo.git",
            "https://github.com/example-org",
            "https://github.com/example-org/nested/repo.git",
            "/srv/git/example-repo.git",
            "",
        ] {
            assert_eq!(
                canonical_id_of_remote(url),
                None,
                "{url} is not a GitHub repository this build can manage"
            );
        }
    }

    #[test]
    fn remote_urls_are_built_from_the_display_names() {
        assert_eq!(
            ssh_remote_url("Example-Org", "Example-Repo"),
            "git@github.com:Example-Org/Example-Repo.git"
        );
        assert_eq!(
            https_remote_url("Example-Org", "Example-Repo"),
            "https://github.com/Example-Org/Example-Repo.git"
        );
        // 表記が違っても、正規化すれば同じ案件を指す。
        assert_eq!(
            canonical_id_of_remote(&ssh_remote_url("Example-Org", "Example-Repo")),
            canonical_id_of_remote(&https_remote_url("example-org", "example-repo"))
        );
    }

    #[test]
    fn remote_tracking_refs_round_trip_through_the_branch_name() {
        assert_eq!(origin_ref("develop"), "refs/remotes/origin/develop");
        assert_eq!(
            branch_of_origin_ref("refs/remotes/origin/develop"),
            Some("develop")
        );
        assert_eq!(
            branch_of_origin_ref("refs/remotes/origin/feature/login"),
            Some("feature/login")
        );
        for value in [
            "develop",
            "refs/heads/develop",
            "refs/remotes/upstream/develop",
            "refs/remotes/origin/",
            "refs/remotes/origin/-delete",
        ] {
            assert_eq!(
                branch_of_origin_ref(value),
                None,
                "{value} is not a remote-tracking ref of origin"
            );
        }
    }
}
