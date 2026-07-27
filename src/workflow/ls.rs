//! `sbxm ls`。
//!
//! 管理案件と、管理外のSandboxを別のtableで一覧する。取り込みも削除も行わない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::Result;

use super::inventory::{self, ProjectState};

/// 一覧の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project: String,
    pub sandbox: String,
    pub state: ProjectState,
}

/// 管理外Sandboxの1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmanagedRow {
    pub sandbox: String,
    pub state: String,
    pub workspace: String,
}

/// `ls`の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub projects: Vec<ProjectRow>,
    pub unmanaged: Vec<UnmanagedRow>,
}

/// 管理案件と管理外Sandboxを一覧する。
pub fn run(
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<Listing> {
    let inventory = inventory::take(config, host, workspace_root)?;

    Ok(Listing {
        projects: inventory
            .projects
            .iter()
            .map(|project| ProjectRow {
                project: project.display_id(),
                sandbox: project.sandbox.as_str().to_string(),
                state: project.state,
            })
            .collect(),
        unmanaged: inventory
            .unmanaged
            .iter()
            .map(|entry| UnmanagedRow {
                sandbox: entry.name.clone(),
                state: match entry.state {
                    crate::compatibility::SandboxState::Running => "running".to_string(),
                    crate::compatibility::SandboxState::Stopped => "stopped".to_string(),
                },
                // 観測できない項目を推測で埋めない。
                workspace: entry.workspace.clone().unwrap_or_else(|| "-".to_string()),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorId;
    use crate::workflow::inventory::tests::{FakeSbx, fixture};

    #[test]
    fn managed_projects_and_unmanaged_sandboxes_are_listed_separately() {
        let fixture = fixture();
        let first = fixture.register("Example-Org/Example-Repo");
        let second = fixture.register("other/repo");
        let host = FakeSbx::listing(&format!(
            r#"[{},{},{{"name":"sbxm-foreign","state":"running","workspace":"/tmp/elsewhere","template":"other:1"}}]"#,
            fixture.entry(&first, "running"),
            fixture.entry(&second, "stopped"),
        ));

        let listing = run(&fixture.config, &host, &fixture.workspace_root).expect("list");
        assert_eq!(
            listing.projects,
            vec![
                ProjectRow {
                    project: "Example-Org/Example-Repo".to_string(),
                    sandbox: first.sandbox.as_str().to_string(),
                    state: ProjectState::Running,
                },
                ProjectRow {
                    project: "other/repo".to_string(),
                    sandbox: second.sandbox.as_str().to_string(),
                    state: ProjectState::Stopped,
                },
            ]
        );
        assert_eq!(
            listing.unmanaged,
            vec![UnmanagedRow {
                sandbox: "sbxm-foreign".to_string(),
                state: "running".to_string(),
                workspace: "/tmp/elsewhere".to_string(),
            }]
        );
    }

    #[test]
    fn a_project_without_a_sandbox_is_listed_as_not_created() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        let listing = run(
            &fixture.config,
            &FakeSbx::listing("[]"),
            &fixture.workspace_root,
        )
        .expect("list");
        assert_eq!(listing.projects[0].state, ProjectState::NotCreated);
    }

    #[test]
    fn a_host_with_nothing_on_it_still_lists_successfully() {
        let fixture = fixture();
        let listing = run(
            &fixture.config,
            &FakeSbx::listing("[]"),
            &fixture.workspace_root,
        )
        .expect("an empty host is a valid answer");
        assert!(listing.projects.is_empty());
        assert!(listing.unmanaged.is_empty());
    }

    #[test]
    fn a_listing_that_cannot_be_trusted_produces_no_rows() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let host = FakeSbx::listing(&format!(
            r#"[{{"name":"{}","state":"pausing","workspace":"/tmp/x","template":"x"}}]"#,
            project.sandbox
        ));

        let error = run(&fixture.config, &host, &fixture.workspace_root)
            .expect_err("an unknown state stops the listing");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}
