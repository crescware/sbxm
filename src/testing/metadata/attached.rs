use crate::testing::outcome::Checked;

use crate::metadata::{CreationMode, ProjectMetadata, Provisioning};
use crate::testing::project::ssh_repository;
use crate::testing::value::DIGEST;

use super::git_identity;

/// attached modeの1案件。
pub fn attached(owner: &str, repository: &str) -> Checked<ProjectMetadata> {
    Ok(ProjectMetadata {
        repository: ssh_repository(&format!("{owner}/{repository}"))?,
        provisioning: Provisioning {
            mode: CreationMode::Attached,
            start_ref: Some("main".to_string()),
            requested_worktrees: 1,
            dockerfile_sha256: DIGEST.to_string(),
        },
        git_identity: git_identity(),
        rebuild: None,
    })
}
