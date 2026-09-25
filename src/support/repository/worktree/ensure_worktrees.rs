use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::git;
use crate::metadata::{CreationMode, ProjectMetadata};
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
///
/// attachedのworktreeは、originの起点branchの先端から始める。`restored`は作り直した
/// Sandboxへhostから戻したbranchであり、起点branchがその中にあれば、戻したbranchの
/// 先端に立つ。戻したbranchはoriginより先にいることがある。それ以外のbranchが
/// Sandboxに残っていても、originの先端にいなければこの案件の成果物とはみなさない。
/// detachedのworktreeは常にoriginの先端から始める。
pub fn ensure_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    project: &ProjectMetadata,
    branch: &str,
    restored: &[String],
    progress: &mut dyn ProgressSink,
) -> Result<Vec<String>> {
    let git_dir = layout.bare_git_dir();
    let reference = git::origin_ref(branch);
    let origin_commit = sandbox::read(
        host,
        sandbox,
        &["git", "--git-dir", &git_dir, "rev-parse", &reference],
    )?;
    let branch_commit = if restored.iter().any(|name| name == branch) {
        // 戻したばかりのbranchであり、読めなければ無いのではなく読めなかったのである。
        let local = format!("refs/heads/{branch}^{{commit}}");
        sandbox::read(
            host,
            sandbox,
            &[
                "git",
                "--git-dir",
                &git_dir,
                "rev-parse",
                "--verify",
                &local,
            ],
        )?
    } else {
        origin_commit.clone()
    };
    progress.step(msg!("progress-creating-worktrees"));
    for index in 0..project.provisioning.requested_worktrees {
        let path = layout.worktree(index);
        if sandbox::path_exists(host, sandbox, &path)? {
            adopt_worktree(host, sandbox, &git_dir, &path)?;
            continue;
        }
        let mode = mode_for(index, project.provisioning.mode);
        let expected_commit = match mode {
            CreationMode::Attached => &branch_commit,
            CreationMode::Detached => &origin_commit,
        };
        provision_worktree(
            host,
            sandbox,
            &git_dir,
            &path,
            branch,
            mode,
            expected_commit,
        )?;
    }
    Ok(layout.worktree_names(project.provisioning.requested_worktrees))
}
