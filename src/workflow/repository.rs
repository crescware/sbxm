//! Sandbox内のbare repositoryとmanaged worktree。
//!
//! 1 Sandboxにつき1つのbare repositoryを持ち、作業用のworktreeをその下に並べる。
//! 1 treeの場合もbare repositoryとworktreeを分離する。

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::git;
use crate::metadata::{self, CreationMode, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout};

use super::sandbox;

/// このbuildが使うfetch refspec。完全一致だけを再利用の条件とする。
const FETCH_REFSPEC: &str = "+refs/heads/*:refs/remotes/origin/*";

/// bare repositoryを用意する。
///
/// 既存のdirectoryは、対象repositoryのbare cloneであると証明できた場合だけ再利用し、
/// 条件を満たさない場合は自動削除せずに停止する。
pub fn ensure_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
    layout: &SandboxLayout,
) -> Result<()> {
    let git_dir = layout.bare_git_dir();

    if sandbox::path_exists(host, sandbox, &git_dir)? {
        crate::progress::step(&msg!("progress-checking-repository"));
    } else {
        crate::progress::step(&msg!("progress-preparing-repository"));
        sandbox::exec(host, sandbox, &["mkdir", "-p", &layout.bare_root()])?.require_success()?;
        let url = git::https_remote_url(project.owner(), project.repository());
        // `git clone --bare`はremoteのbranchを`refs/heads/*`へ複製する。そのbranchは
        // worktreeを作るときに同じ名前で作ろうとするものと衝突する。bare repositoryは
        // remote-tracking refだけを持つ入れ物として始める。
        sandbox::exec(host, sandbox, &["git", "init", "--bare", &git_dir])?.require_success()?;
        sandbox::exec(
            host,
            sandbox,
            &[
                "git",
                "--git-dir",
                &git_dir,
                "remote",
                "add",
                "origin",
                &url,
            ],
        )?
        .require_success()?;
        sandbox::exec(
            host,
            sandbox,
            &[
                "git",
                "--git-dir",
                &git_dir,
                "config",
                "remote.origin.fetch",
                FETCH_REFSPEC,
            ],
        )?
        .require_success()?;
    }
    verify_bare_clone(host, sandbox, project, &git_dir)?;

    // remote-tracking refを現在の状態にしてから、起点refを解決する。
    crate::progress::step(&msg!("progress-fetching-repository"));
    sandbox::exec_with_progress(
        host,
        sandbox,
        &["git", "--git-dir", &git_dir, "fetch", "--prune", "origin"],
    )?
    .require_success()?;
    Ok(())
}

fn verify_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
    git_dir: &str,
) -> Result<()> {
    let bare = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "rev-parse",
            "--is-bare-repository",
        ],
    )?;
    if bare != "true" {
        return Err(unusable(
            git_dir,
            "the repository is not bare, so it is not the shared repository".to_string(),
        ));
    }

    let urls = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "config",
            "--get-all",
            "remote.origin.url",
        ],
    )?;
    let urls: Vec<&str> = urls
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let [url] = urls.as_slice() else {
        return Err(unusable(
            git_dir,
            format!("origin has {} URLs, so the remote is ambiguous", urls.len()),
        ));
    };
    let canonical = project.canonical();
    match git::canonical_id_of_remote(url) {
        Some(observed) if observed == canonical.as_str() => {}
        Some(observed) => {
            return Err(unusable(
                git_dir,
                format!("origin points at {observed}, not at {canonical}"),
            ));
        }
        None => {
            return Err(unusable(
                git_dir,
                format!("origin URL {url} does not name a GitHub repository"),
            ));
        }
    }

    let refspecs = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "config",
            "--get-all",
            "remote.origin.fetch",
        ],
    )?;
    let refspecs: Vec<&str> = refspecs
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if refspecs != [FETCH_REFSPEC] {
        return Err(unusable(
            git_dir,
            format!(
                "the fetch refspec is {}, not {FETCH_REFSPEC}",
                refspecs.join(", ")
            ),
        ));
    }

    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "--git-dir", git_dir, "fsck", "--connectivity-only"],
    )?;
    if !outcome.success() {
        return Err(unusable(
            git_dir,
            "the repository does not pass a connectivity check".to_string(),
        ));
    }
    Ok(())
}

/// 起点となるbranchを確定させる。
///
/// hostのvalidationは、外部commandへ渡す前に確実に拒否できる条件だけを見る。
/// 起点として使う名前は、Sandbox内のgit自身にもう一度判定させてから採用する。
/// attached modeでremote default branchを解決した場合は、その場でmetadataへ記録する。
pub fn resolve_start_ref(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    paths: &ProjectPaths,
    project: &mut ProjectMetadata,
) -> Result<String> {
    let git_dir = layout.bare_git_dir();

    let stored = project.provisioning.start_ref.clone();
    let branch = match &stored {
        Some(branch) => branch.clone(),
        None => remote_default_branch(host, sandbox, &git_dir)?,
    };
    require_branch_name(host, sandbox, &branch)?;
    if stored.is_none() {
        project.provisioning.start_ref = Some(branch.clone());
        metadata::update(paths, project)?;
    }

    // tagやambiguous refを起点にしないよう、完全なremote-tracking refだけを確認する。
    let reference = git::origin_ref(&branch);
    let outcome = sandbox::exec(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            &git_dir,
            "show-ref",
            "--verify",
            "--quiet",
            &reference,
        ],
    )?;
    if !outcome.success() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::StartRefUnresolved,
                msg!(
                    "error-start-ref-unresolved",
                    reference = reference,
                    project = project.display_id()
                ),
            )
            .remediation(msg!("remediation-start-ref-unresolved")),
        ));
    }
    Ok(branch)
}

/// 起点branch名を、Sandbox内の`git check-ref-format --branch`で再検証する。
///
/// repositoryを指定せずに実行するため、`@{-1}`のような文脈依存の短縮形は展開されず、
/// branch名としてそのまま判定される。
fn require_branch_name(host: &dyn HostEnvironment, sandbox: &str, branch: &str) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "check-ref-format", "--branch", branch],
    )?;
    if outcome.success() {
        return Ok(());
    }
    fail(
        ErrorId::InvalidBranchName,
        msg!(
            "error-invalid-branch-name",
            value = branch,
            detail = "git in the sandbox does not accept this as a branch name"
        ),
    )
}

/// `git ls-remote --symref origin HEAD`が示すdefault branch。
fn remote_default_branch(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
) -> Result<String> {
    let output = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "ls-remote",
            "--symref",
            "origin",
            "HEAD",
        ],
    )?;

    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix("ref:") else {
            continue;
        };
        let Some(reference) = rest.split_whitespace().next() else {
            continue;
        };
        // branch以外がHEADの指す先である場合は受け付けない。
        if let Some(branch) = reference.strip_prefix("refs/heads/")
            && git::validate_branch_name(branch).is_ok()
        {
            return Ok(branch.to_string());
        }
    }

    Err(Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = "git ls-remote --symref",
            detail = "no branch was reported for HEAD"
        ),
    ))
}

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
) -> Result<Vec<String>> {
    let git_dir = layout.bare_git_dir();
    let reference = git::origin_ref(branch);
    let expected_commit = sandbox::read(
        host,
        sandbox,
        &["git", "--git-dir", &git_dir, "rev-parse", &reference],
    )?;
    crate::progress::step(&msg!("progress-creating-worktrees"));
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

fn create_worktree(
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
fn mode_for(index: u32, project: CreationMode) -> CreationMode {
    match index {
        0 => project,
        _ => CreationMode::Detached,
    }
}

/// この実行で用意するworktreeを、起点commitの上に立たせる。
///
/// 中断した作成が残した成果物は作り直さず引き継ぐ。作ったばかりのworktreeは起点commit
/// にいるはずであり、そこにいないものはこの案件の成果物ではない。
fn provision_worktree(
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
fn adopt_worktree(
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
fn verify_mode(
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

/// 成果物を自動削除せず、観測した不一致を示して停止する。
fn unusable(path: &str, detail: String) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::SandboxRepositoryUnusable,
            msg!(
                "error-sandbox-repository-unusable",
                path = path,
                detail = detail
            ),
        )
        .remediation(msg!("remediation-sandbox-repository-unusable", path = path)),
    )
}

#[cfg(test)]
#[path = "repository_test.rs"]
mod repository_test;
