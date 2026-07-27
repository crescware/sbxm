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
