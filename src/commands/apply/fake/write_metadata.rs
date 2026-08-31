use crate::testing::outcome::{Checked, Required};

use crate::config::ConfigLocation;
use crate::metadata::{self, CreationMode, ProjectMetadata, Provisioning, RebuildIntent};
use crate::paths::{ProjectParent, ProjectPaths};
use crate::registry::{RegistryEntry, RegistryGuard};
use crate::testing::value::DIGEST;

use super::canonical;

pub fn write_metadata(
    location: &ConfigLocation,
    parent: &ProjectParent,
    rebuild: Option<RebuildIntent>,
) -> Checked<ProjectPaths> {
    let paths = ProjectPaths::derive(parent, &canonical()?);
    std::fs::create_dir_all(paths.sbxm_dir()).required()?;
    let repository = crate::testing::project::ssh_repository("Example-Org/Example-Repo")?;
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
        rebuild,
    };
    metadata::create(&paths, &metadata).required()?;
    Ok(paths)
}
