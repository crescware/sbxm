use crate::boundary::host::HostEnvironment;
use crate::design::ProgressSink;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::{Initialization, SandboxOrigin};

/// bare repositoryが、初期化の途中で中断した状態かを、変更せずに観測する。
///
/// 内容があるrepository、別のrepositoryを指すorigin、複数のoriginは、中断の跡と
/// みなさない。続きを補う`ensure_bare_clone`と、欠けを判定する観測は、この同じ規則を
/// 使う。片方だけが中断の跡と認めると、観測が拒否したまま補う工程へ届かない。
pub fn observe_initialization(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    origin: &SandboxOrigin,
    git_dir: &str,
    progress: &mut dyn ProgressSink,
) -> Result<Initialization> {
    let bare = super::inspect_bare(host, sandbox_name, git_dir, progress)?.require_success()?;
    if bare.stdout_text().trim() != "true" {
        return Ok(Initialization::Settled);
    }
    let configured = sandbox::exec(
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
    let origin_urls = configured.stdout_text();
    if !origin_urls.trim().is_empty() {
        return fetch_refspec_missing(host, sandbox_name, origin, git_dir, &origin_urls);
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
        return Ok(Initialization::Settled);
    }
    Ok(Initialization::OriginMissing)
}

/// origin設定済みで、`remote.origin.fetch`だけ欠けているか。
///
/// `remote add origin`の直後、`config remote.origin.fetch`より前に中断した状態を
/// 想定する。originが対象repositoryと一致し、fetch refspecが未設定の場合だけ
/// 中断の跡とし、別repositoryのoriginや複数originは中断の跡とみなさない。
fn fetch_refspec_missing(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    origin: &SandboxOrigin,
    git_dir: &str,
    origin_urls: &str,
) -> Result<Initialization> {
    let urls: Vec<&str> = origin_urls
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let [url] = urls.as_slice() else {
        return Ok(Initialization::Settled);
    };
    if origin.verify(url).is_err() {
        return Ok(Initialization::Settled);
    }
    let fetch = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            git_dir,
            "config",
            "--get-all",
            "remote.origin.fetch",
        ],
    )?;
    if !fetch.stdout_text().trim().is_empty() {
        return Ok(Initialization::Settled);
    }
    Ok(Initialization::FetchRefspecMissing)
}
