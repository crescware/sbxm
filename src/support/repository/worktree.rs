//! managed worktreeの作成と、既存treeの引き受け。

use crate::command::HostEnvironment;
use crate::error::Result;
use crate::git;
use crate::metadata::{CreationMode, ProjectMetadata};
use crate::msg;
use crate::project::SandboxLayout;

use super::sandbox;

use super::unusable;
use crate::ui::ProgressSink;

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

pub(super) fn create_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    path: &str,
    branch: &str,
    mode: CreationMode,
) -> Result<()> {
    let reference = git::origin_ref(branch);
    let arguments: Vec<&str> = match mode {
        CreationMode::Attached => vec![
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "add",
            "--track",
            "-b",
            branch,
            path,
            &reference,
        ],
        CreationMode::Detached => vec![
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "add",
            "--detach",
            path,
            &reference,
        ],
    };
    sandbox::exec(host, sandbox, &arguments)?.require_success()?;
    Ok(())
}

/// これから作るworktreeのmode。
///
/// Gitは同じbranchを2つのworktreeへcheckoutさせない。attachedなworktreeは案件に1つしか
/// 持てないため、案件のmodeが効くのは最初の1本だけである。2本目以降はdetachedとする。
pub(super) fn mode_for(index: u32, project: CreationMode) -> CreationMode {
    match index {
        0 => project,
        _ => CreationMode::Detached,
    }
}

/// この実行で用意するworktreeを、起点commitの上に立たせる。
///
/// 中断した作成が残した成果物は作り直さず引き継ぐ。作ったばかりのworktreeは起点commit
/// にいるはずであり、そこにいないものはこの案件の成果物ではない。
pub(super) fn provision_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    path: &str,
    branch: &str,
    mode: CreationMode,
    expected_commit: &str,
) -> Result<()> {
    if !sandbox::path_exists(host, sandbox, path)? {
        create_worktree(host, sandbox, git_dir, path, branch, mode)?;
    }
    let head = sandbox::read(host, sandbox, &["git", "-C", path, "rev-parse", "HEAD"])?;
    if head != expected_commit {
        return Err(unusable(
            path,
            format!("HEAD is {head}, and this project starts from {expected_commit}"),
        ));
    }
    verify_mode(host, sandbox, path, branch, mode)
}

/// 既に案件の成果物として記録済みのworktreeを、そのまま引き継ぐ。
///
/// 求めるのは、この共有repositoryのworktreeであり続けていることだけとする。
///
/// 起点commitもmodeも条件にしない。そこで作業するためのworktreeであり、commitすれば
/// HEADは動き、branchを切ればmodeも変わる。そこで起きたことを異常として扱うと、
/// 作業した案件はworktreeを増やせなくなる。どちらもsbxmが作るときの事後条件であって、
/// 既にあるものへの要件ではない。
pub(super) fn adopt_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    path: &str,
) -> Result<()> {
    // `--path-format=absolute`を付けないと、gitは条件によって相対pathを返す。bare git
    // dirとの一致を見る比較では、返る形が決まっていないと判定にならない。
    let common = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "-C",
            path,
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ],
    )?;
    if common != git_dir {
        return Err(unusable(
            path,
            format!("the worktree belongs to {common}, not to {git_dir}"),
        ));
    }
    Ok(())
}

/// worktreeが宣言どおりのmodeであることを確認する。
pub(super) fn verify_mode(
    host: &dyn HostEnvironment,
    sandbox: &str,
    path: &str,
    branch: &str,
    mode: CreationMode,
) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "-C", path, "symbolic-ref", "-q", "HEAD"],
    )?;
    let observed = outcome.stdout_text().trim().to_string();
    match mode {
        CreationMode::Attached => {
            let expected = format!("refs/heads/{branch}");
            if !outcome.success() || observed != expected {
                return Err(unusable(path, format!("the worktree is not on {expected}")));
            }
        }
        CreationMode::Detached => {
            if outcome.success() {
                return Err(unusable(
                    path,
                    format!("the worktree is on {observed}, and this project uses detached heads"),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;
