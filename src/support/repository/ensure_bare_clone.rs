use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::git;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout};

use crate::design::ProgressSink;
use crate::support::sandbox;

use super::{FETCH_REFSPEC, TagFollowing, verify_bare_clone};

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
        complete_empty_initialization(host, sandbox, project, &git_dir)?;
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
    super::refresh_origin(host, sandbox, &git_dir, TagFollowing::Auto, Some(progress))?
        .require_success()?;
    Ok(())
}

/// `git init --bare`の直後に中断した、まだ利用者のrefもobjectも無いrepositoryだけを
/// 宣言済みoriginへ進める。内容があるdirectoryには設定を足さない。
fn complete_empty_initialization(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    project: &ProjectId,
    git_dir: &str,
) -> Result<()> {
    let bare = sandbox::read(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "rev-parse",
            "--is-bare-repository",
        ],
    )?;
    if bare != "true" {
        return Ok(());
    }
    let origin = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "config",
            "--get-all",
            "remote.origin.url",
        ],
    )?;
    if !origin.stdout_text().trim().is_empty() {
        return Ok(());
    }
    let refs = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "for-each-ref",
            "--format=%(refname)",
        ],
    )?
    .require_success()?;
    let objects = sandbox::exec(
        host,
        sandbox_name,
        &["git", "--git-dir", git_dir, "count-objects", "-v"],
    )?
    .require_success()?;
    let empty_objects = objects.stdout_text().lines().any(|line| line == "count: 0")
        && objects
            .stdout_text()
            .lines()
            .any(|line| line == "in-pack: 0");
    if !refs.stdout_text().trim().is_empty() || !empty_objects {
        return Ok(());
    }
    let url = git::https_remote_url(project.owner(), project.repository());
    sandbox::exec(
        host,
        sandbox_name,
        &["git", "--git-dir", git_dir, "remote", "add", "origin", &url],
    )?
    .require_success()?;
    sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "config",
            "remote.origin.fetch",
            FETCH_REFSPEC,
        ],
    )?
    .require_success()?;
    Ok(())
}
