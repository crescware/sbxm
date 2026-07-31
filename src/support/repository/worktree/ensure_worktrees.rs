use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::git;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::SandboxLayout;

use crate::support::sandbox;

use crate::design::ProgressSink;

use super::{adopt_worktree, mode_for, provision_worktree};

/// managed worktreeを、indexを固定したまま用意する。
///
/// 既にあるworktreeは、起点commitともmodeとも照らさずに引き継ぐ。そこで作業するための
/// worktreeであり、commitすればHEADは動き、branchを切ればmodeも変わる。どちらもsbxmが
/// 作るときの事後条件であって、既にあるものへの要件ではない。
pub fn ensure_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    project: &ProjectMetadata,
    branch: &str,
    progress: &mut dyn ProgressSink,
) -> Result<Vec<String>> {
    let git_dir = layout.bare_git_dir();
    let reference = git::origin_ref(branch);
    let expected_commit = sandbox::read(
        host,
        sandbox,
        &["git", "--git-dir", &git_dir, "rev-parse", &reference],
    )?;
    progress.step(msg!("progress-creating-worktrees"));
    for index in 0..project.provisioning.requested_worktrees {
        let path = layout.worktree(index);
        if sandbox::path_exists(host, sandbox, &path)? {
            adopt_worktree(host, sandbox, &git_dir, &path)?;
            continue;
        }
        provision_worktree(
            host,
            sandbox,
            &git_dir,
            &path,
            branch,
            mode_for(index, project.provisioning.mode),
            &expected_commit,
        )?;
    }
    Ok(layout.worktree_names(project.provisioning.requested_worktrees))
}
