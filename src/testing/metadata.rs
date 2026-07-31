//! Project metadataのtestが共有するfixture。

use crate::metadata::{CreationMode, GitIdentity, ProjectMetadata, Provisioning};
use crate::project::{CanonicalProjectId, ProjectId};
use crate::testing::project::ssh_repository;
use crate::testing::value::DIGEST;

/// `DIGEST`とは別の世代を指す固定値。
pub const OTHER_DIGEST: &str = "2222222222222222222222222222222222222222222222222222222222222222";

pub fn canonical(value: &str) -> CanonicalProjectId {
    ProjectId::parse(value)
        .expect("valid project id")
        .canonical()
}

/// testが使うhost由来のGit identity。
pub fn git_identity() -> GitIdentity {
    GitIdentity {
        user_name: "Example User".to_string(),
        user_email: "user@example.com".to_string(),
    }
}

/// attached modeの1案件。
pub fn attached(owner: &str, repository: &str) -> ProjectMetadata {
    ProjectMetadata {
        repository: ssh_repository(&format!("{owner}/{repository}")),
        provisioning: Provisioning {
            mode: CreationMode::Attached,
            start_ref: Some("main".to_string()),
            requested_worktrees: 1,
            dockerfile_sha256: DIGEST.to_string(),
        },
        git_identity: git_identity(),
        rebuild: None,
    }
}
