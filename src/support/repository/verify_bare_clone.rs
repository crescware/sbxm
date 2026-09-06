use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::git;
use crate::msg;
use crate::project::ProjectId;

use crate::support::sandbox;

use super::{FETCH_REFSPEC, unusable};

/// 既存のbare repositoryが案件のrepositoryとして再利用できるかを観測する。
pub fn verify_bare_clone(
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
    let canonical = project.canonical();
    match git::canonical_id_of_remote(url) {
        Some(observed) if observed == canonical.as_str() => {}
        Some(observed) => {
            return Err(unusable(
                git_dir,
                msg!(
                    "cause-origin-elsewhere",
                    observed = observed,
                    declared = canonical
                ),
            ));
        }
        None => {
            return Err(unusable(
                git_dir,
                msg!("cause-origin-not-a-github-repository", observed = url),
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
