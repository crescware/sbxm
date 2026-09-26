use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::msg;

use crate::support::sandbox;

use super::{FETCH_REFSPEC, SandboxOrigin, unusable, unusable_host_origin};

/// 既存のbare repositoryが案件のrepositoryとして再利用できるかを観測する。
pub fn verify_bare_clone(
    host: &dyn HostEnvironment,
    sandbox: &str,
    origin: &SandboxOrigin,
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
        return Err(unusable(git_dir, msg!("cause-not-bare-repository")));
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
            msg!("cause-origin-ambiguous", count = urls.len()),
        ));
    };
    // hostにあるrepositoryの案件でoriginが違えば、作り直しを案内する。以前のsbxmが作った
    // Sandboxは、originを別の場所へ向けている。
    origin.verify(url).map_err(|reason| match origin {
        SandboxOrigin::Host { project, .. } => unusable_host_origin(git_dir, reason, project),
        SandboxOrigin::Github(_) => unusable(git_dir, reason),
    })?;

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
            msg!(
                "cause-fetch-refspec-differs",
                observed = refspecs.join(", "),
                expected = FETCH_REFSPEC
            ),
        ));
    }

    let outcome = sandbox::exec(
        host,
        sandbox,
        &["git", "--git-dir", git_dir, "fsck", "--connectivity-only"],
    )?;
    if !outcome.success() {
        return Err(unusable(git_dir, msg!("cause-connectivity-check-failed")));
    }
    Ok(())
}
