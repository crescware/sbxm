//! Project識別子。
//!
//! stringを直接workflowへ渡さず、validation済みの型を使う。

use crate::error::{Error, ErrorId, Result, fail};
use crate::msg;

/// `<owner>/<repository>`。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectId {
    value: String,
}

impl ProjectId {
    /// 1個のslashを持つ`<owner>/<repository>`を検証する。
    pub fn parse(value: &str) -> Result<ProjectId> {
        let invalid = || {
            Error::new(
                ErrorId::InvalidProjectId,
                msg!("error-invalid-project-id", value = value),
            )
        };

        let mut parts = value.split('/');
        let (owner, repository) = match (parts.next(), parts.next(), parts.next()) {
            (Some(owner), Some(repository), None) => (owner, repository),
            _ => return Err(invalid()),
        };

        if !is_valid_owner(owner) || !is_valid_repository(repository) {
            return Err(invalid());
        }
        if repository == "." || repository == ".." {
            return Err(invalid());
        }
        // `.sbxm`はsbxmが予約する名前である。
        if repository.eq_ignore_ascii_case(".sbxm") {
            return fail(
                ErrorId::ReservedRepositoryName,
                msg!("error-reserved-repository-name", value = repository),
            );
        }

        Ok(ProjectId {
            value: value.to_string(),
        })
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

/// `[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?`
fn is_valid_owner(owner: &str) -> bool {
    let bytes = owner.as_bytes();
    if bytes.is_empty() || bytes.len() > 39 {
        return false;
    }
    let alphanumeric = |b: u8| b.is_ascii_alphanumeric();
    if !alphanumeric(bytes[0]) || !alphanumeric(bytes[bytes.len() - 1]) {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
}

/// `[A-Za-z0-9._-]{1,100}`
fn is_valid_repository(repository: &str) -> bool {
    let bytes = repository.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_ids_keep_the_given_casing() {
        let id = ProjectId::parse("Example-Org/Example.Repo").expect("valid");
        assert_eq!(id.to_string(), "Example-Org/Example.Repo");
    }

    #[test]
    fn project_ids_require_exactly_one_slash() {
        for value in ["owner", "owner/repo/extra", "/repo", "owner/", ""] {
            let error = ProjectId::parse(value).expect_err("{value} must be rejected");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::InvalidProjectId),
                "value {value} produced the wrong error"
            );
        }
    }

    #[test]
    fn owner_and_repository_character_rules_are_enforced() {
        assert!(ProjectId::parse("a/b").is_ok());
        assert!(ProjectId::parse("a-b-c/repo_name.v2").is_ok());
        assert!(ProjectId::parse(&format!("{}/repo", "a".repeat(39))).is_ok());
        assert!(ProjectId::parse(&format!("{}/repo", "a".repeat(40))).is_err());
        assert!(ProjectId::parse("-owner/repo").is_err());
        assert!(ProjectId::parse("owner-/repo").is_err());
        assert!(ProjectId::parse("own.er/repo").is_err());
        assert!(ProjectId::parse(&format!("owner/{}", "r".repeat(100))).is_ok());
        assert!(ProjectId::parse(&format!("owner/{}", "r".repeat(101))).is_err());
        assert!(ProjectId::parse("owner/re po").is_err());
    }

    #[test]
    fn dot_and_dot_dot_are_not_repository_names() {
        assert!(ProjectId::parse("owner/.").is_err());
        assert!(ProjectId::parse("owner/..").is_err());
    }

    #[test]
    fn the_reserved_repository_name_is_rejected_case_insensitively() {
        for value in ["owner/.sbxm", "owner/.SBXM", "owner/.Sbxm"] {
            let error = ProjectId::parse(value).expect_err("reserved names are rejected");
            assert_eq!(error.first_id(), Some(ErrorId::ReservedRepositoryName));
        }
    }
}
