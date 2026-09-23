use crate::testing::outcome::{Checked, Required};

use crate::config::ConfigLocation;
use crate::metadata::{self, CreationMode, ProjectMetadata, Provisioning, RebuildIntent};
use crate::paths::{ProjectParent, ProjectPaths};
use crate::project::ProjectId;
use crate::registry::{RegistryEntry, RegistryGuard};
use crate::testing::value::DIGEST;

/// 案件を、registry entryとmetadataの両方が揃った状態にする。
pub fn register_project(
    location: &ConfigLocation,
    parent: &ProjectParent,
    project: &str,
    rebuild: Option<RebuildIntent>,
) -> Checked<ProjectPaths> {
    let paths = ProjectPaths::derive(parent, &ProjectId::parse(project).required()?.canonical());
    std::fs::create_dir_all(paths.sbxm_dir()).required()?;
    let repository = crate::testing::project::ssh_repository(project)?;
    let mut guard = RegistryGuard::acquire(location).required()?;
    guard
        .insert(RegistryEntry::new(paths.root(), repository.clone()).required()?)
        .required()?;
    drop(guard);
    let metadata = ProjectMetadata {
        repository,
        provisioning: Provisioning {
            mode: CreationMode::Attached,
            start_ref: Some("main".into()),
            requested_worktrees: 1,
            dockerfile_sha256: DIGEST.into(),
        },
        git_identity: crate::testing::metadata::git_identity(),
        initial_provisioning: None,
        declared_files: None,
        rebuild,
    };
    metadata::create(&paths, &metadata).required()?;
    Ok(paths)
}
