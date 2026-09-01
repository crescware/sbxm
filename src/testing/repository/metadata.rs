use crate::testing::outcome::Checked;

use crate::metadata::{CreationMode, ProjectMetadata, Provisioning};

pub fn metadata(
    mode: CreationMode,
    start_ref: Option<&str>,
    count: u32,
) -> Checked<ProjectMetadata> {
    Ok(ProjectMetadata {
        repository: crate::testing::project::ssh_repository("Example-Org/Example-Repo")?,
        provisioning: Provisioning {
            mode,
            start_ref: start_ref.map(std::string::ToString::to_string),
            requested_worktrees: count,
            dockerfile_sha256: "1".repeat(64),
        },
        git_identity: crate::testing::metadata::git_identity(),
        initial_provisioning: None,
        declared_files: None,
        rebuild: None,
    })
}
