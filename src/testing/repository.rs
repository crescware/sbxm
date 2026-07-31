//! `support::repository`のtestが共有するfixture。

use crate::testing::outcome::{Checked, Required};

use crate::metadata::{CreationMode, ProjectMetadata, Provisioning};
use crate::paths::{ProjectParent, ProjectPaths};
use crate::project::{CanonicalProjectId, ProjectId, SandboxLayout};
use crate::support::repository::FETCH_REFSPEC;
use crate::testing::project::project_id;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

pub fn project() -> Checked<ProjectId> {
    project_id("Example-Org/Example-Repo")
}

pub fn canonical() -> Checked<CanonicalProjectId> {
    Ok(project()?.canonical())
}

pub fn layout() -> Checked<SandboxLayout> {
    Ok(SandboxLayout::new(&canonical()?))
}

/// bare cloneの検査を通る応答。
pub fn healthy_clone() -> Checked<InnerCommandSandbox> {
    let git_dir = layout()?.bare_git_dir();
    Ok(InnerCommandSandbox::new()
        .answering(
            &format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
            "true\n",
        )
        .answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
            "https://github.com/Example-Org/Example-Repo.git\n",
        )
        .answering(
            &format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            &format!("{FETCH_REFSPEC}\n"),
        ))
}

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
        rebuild: None,
    })
}

pub fn project_paths(dir: &std::path::Path) -> Checked<ProjectPaths> {
    let parent = ProjectParent::at(dir).required_because("valid parent directory")?;
    let paths = ProjectPaths::derive(&parent, &canonical()?);
    std::fs::create_dir_all(paths.sbxm_dir()).required_because("create .sbxm")?;
    Ok(paths)
}

/// worktreeの検査を通る応答。
pub fn worktree_host(mode: CreationMode, count: u32) -> Checked<InnerCommandSandbox> {
    let git_dir = layout()?.bare_git_dir();
    let mut host = InnerCommandSandbox::new().answering(
        &format!("git --git-dir {git_dir} rev-parse refs/remotes/origin/develop"),
        &format!("{COMMIT}\n"),
    );
    for index in 0..count {
        let path = layout()?.worktree(index);
        host = host.answering(
            &format!("git -C {path} rev-parse HEAD"),
            &format!("{COMMIT}\n"),
        );
        host = host.answering(
            &format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
            &format!("{}\n", layout()?.bare_git_dir()),
        );
        host = match mode {
            CreationMode::Attached => host.answering(
                &format!("git -C {path} symbolic-ref -q HEAD"),
                "refs/heads/develop\n",
            ),
            CreationMode::Detached => host.failing(&format!("git -C {path} symbolic-ref -q HEAD")),
        };
    }
    Ok(host)
}
