//! Sandbox内のbare repositoryとmanaged worktree。
//!
//! 1 Sandboxにつき1つのbare repositoryを持ち、作業用のworktreeをその下に並べる。
//! 1 treeの場合もbare repositoryとworktreeを分離する。

mod start_ref;
mod worktree;

pub use start_ref::resolve_start_ref;
pub use worktree::ensure_worktrees;

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::git;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use super::sandbox;
use crate::ui::ProgressSink;

/// このbuildが使うfetch refspec。完全一致だけを再利用の条件とする。
pub(crate) const FETCH_REFSPEC: &str = "+refs/heads/*:refs/remotes/origin/*";
/// bare repositoryを用意する。
///
/// 既存のdirectoryは、対象repositoryのbare cloneであると証明できた場合だけ再利用し、
/// 条件を満たさない場合は自動削除せずに停止する。
pub fn ensure_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
    layout: &SandboxLayout,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let git_dir = layout.bare_git_dir();

    if sandbox::path_exists(host, sandbox, &git_dir)? {
        progress.step(msg!("progress-checking-repository"));
    } else {
        progress.step(msg!("progress-preparing-repository"));
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
    progress.step(msg!("progress-fetching-repository"));
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
#[path = "mod_test.rs"]
mod mod_test;
