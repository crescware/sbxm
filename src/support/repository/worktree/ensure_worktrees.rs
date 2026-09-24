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
/// 起点branchがすでにSandboxのbranchとしてあれば、attachedのworktreeはその先端に立つ。
/// 作り直したSandboxへhostから戻したbranchがこれにあたる。detachedのworktreeは常に
/// originの先端から始める。
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
    let origin_commit = sandbox::read(
        host,
        sandbox,
        &["git", "--git-dir", &git_dir, "rev-parse", &reference],
    )?;
    let branch_commit =
        local_branch_tip(host, sandbox, &git_dir, branch)?.unwrap_or_else(|| origin_commit.clone());
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

/// 起点branchがSandboxのbranchとしてあれば、その先端。
fn local_branch_tip(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    branch: &str,
) -> Result<Option<String>> {
    let local = format!("refs/heads/{branch}^{{commit}}");
    let outcome = sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "rev-parse",
            "--verify",
            "--quiet",
            &local,
        ],
    )?;
    let tip = outcome.stdout_text().trim().to_string();
    // 無いbranchは、終了statusと空の出力で答える。
    Ok((outcome.success() && !tip.is_empty()).then_some(tip))
}
