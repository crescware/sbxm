//! Project metadataのtestが共有するfixture。

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::metadata::{CreationMode, ProjectMetadata, Provisioning};
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
        rebuild: None,
    }
}

/// 導出どおりのpathへmetadataを書き、project rootを返す。
pub fn write_project(base: &Path, owner: &str, repository: &str, text: &str) -> PathBuf {
    let root = base
        .join(owner.to_ascii_lowercase())
        .join(format!("{}.project", repository.to_ascii_lowercase()));
    let dir = root.join(".sbxm");
    std::fs::create_dir_all(&dir).expect("create .sbxm");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let path = dir.join("project.yaml");
    std::fs::write(&path, text).expect("write metadata");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("mode");
    root
}
