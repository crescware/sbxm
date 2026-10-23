use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::SandboxLayout;

use crate::design::ProgressSink;
use crate::support::sandbox::{self, Live};

use super::{
    FETCH_REFSPEC, Initialization, SandboxOrigin, observe_initialization, settle_refusals,
    verify_bare_clone,
};

/// bare repositoryを用意する。
///
/// 既存のdirectoryは、対象repositoryのbare cloneであると証明できた場合だけ再利用し、
/// 条件を満たさない場合は自動削除せずに停止する。hostにあるrepositoryは、hostのgitが
/// ssh越しにoriginを書き込む。
pub fn ensure_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    origin: &SandboxOrigin,
    layout: &SandboxLayout,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let git_dir = layout.bare_git_dir();

    if sandbox::path_exists(&Live::new(host, sandbox), &git_dir)? {
        complete_initialization(host, sandbox, origin, &git_dir, progress)?;
    } else {
        progress.step(msg!("progress-preparing-repository"));
        sandbox::exec(host, sandbox, &["mkdir", "-p", &layout.bare_root()])?.require_success()?;
        // `git clone --bare`はremoteのbranchを`refs/heads/*`へ複製する。そのbranchは
        // worktreeを作るときに同じ名前で作ろうとするものと衝突する。bare repositoryは
        // remote-tracking refだけを持つ入れ物として始める。
        sandbox::exec(host, sandbox, &["git", "init", "--bare", &git_dir])?.require_success()?;
        add_origin(host, sandbox, origin, &git_dir)?;
        set_fetch_refspec(host, sandbox, &git_dir)?;
    }
    verify_bare_clone(host, sandbox, origin, &git_dir, progress)?;

    // remote-tracking refを現在の状態にしてから、起点refを解決する。hostにあるrepositoryは、
    // Sandboxの中でfetchせず、hostのgitが書き込む。
    progress.step(match origin {
        SandboxOrigin::Github(_) => msg!("progress-fetching-repository"),
        SandboxOrigin::Host { .. } => msg!("progress-sending-repository"),
    });
    let refused = origin.refresh(host, sandbox, &git_dir, Some(&mut *progress))?;
    settle_refusals(sandbox, &refused, progress)
}

/// 初期化の途中で中断したrepositoryだけを、残りの工程まで進める。内容があるdirectoryには
/// 設定を足さない。
fn complete_initialization(
    host: &dyn HostEnvironment,
    sandbox: &str,
    origin: &SandboxOrigin,
    git_dir: &str,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    match observe_initialization(&Live::new(host, sandbox), origin, git_dir, progress)? {
        Initialization::Settled => Ok(()),
        Initialization::OriginMissing => {
            add_origin(host, sandbox, origin, git_dir)?;
            set_fetch_refspec(host, sandbox, git_dir)
        }
        Initialization::FetchRefspecMissing => set_fetch_refspec(host, sandbox, git_dir),
    }
}

fn add_origin(
    host: &dyn HostEnvironment,
    sandbox: &str,
    origin: &SandboxOrigin,
    git_dir: &str,
) -> Result<()> {
    let url = origin.url();
    sandbox::exec(
        host,
        sandbox,
        &["git", "--git-dir", git_dir, "remote", "add", "origin", &url],
    )?
    .require_success()?;
    Ok(())
}

fn set_fetch_refspec(host: &dyn HostEnvironment, sandbox: &str, git_dir: &str) -> Result<()> {
    sandbox::exec(
        host,
        sandbox,
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
