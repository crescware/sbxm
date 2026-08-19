use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::project::{ProjectId, SandboxLayout};

use crate::support::sandbox;

use super::{verify_bare_clone, worktree};

/// 既存repositoryとmanaged worktreeをread-onlyで確認する。
pub(crate) fn verify_existing(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    project: &ProjectId,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
) -> Result<()> {
    let git_dir = layout.bare_git_dir();
    if !sandbox::path_exists(host, sandbox_name, &git_dir)? {
        return Ok(());
    }
    verify_bare_clone(host, sandbox_name, project, &git_dir)?;
    for name in layout.worktree_names(metadata.provisioning.requested_worktrees) {
        let path = format!("{}/{name}", layout.bare_root());
        if sandbox::path_exists(host, sandbox_name, &path)? {
            worktree::adopt_worktree(host, sandbox_name, &git_dir, &path)?;
        }
    }
    Ok(())
}
