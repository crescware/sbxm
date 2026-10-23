use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;

use crate::support::sandbox::Inside;

use super::{FETCH_REFSPEC, SandboxOrigin, unusable, unusable_host_origin};

/// 既存のbare repositoryの設定が、案件のrepositoryのものかを観測する。
///
/// bareであること、originが1つで案件のrepositoryを指すこと、fetch refspecがsbxmの
/// 書くものであることを確かめる。objectの接続性は見ない。それは`verify_bare_clone`が、
/// repositoryを作るときと同期するときに加える。
///
/// 読むcommandは`inspection_commands`に含まれる。
pub fn verify_bare_config(
    inside: &dyn Inside,
    origin: &SandboxOrigin,
    git_dir: &str,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let bare = super::inspect_bare(inside, git_dir, progress)?.require_success()?;
    if bare.stdout_text().trim() != "true" {
        return Err(unusable(git_dir, msg!("cause-not-bare-repository")));
    }

    progress.step(msg!("progress-inspect-repository-config"));
    let urls = read(
        inside,
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

    let refspecs = read(
        inside,
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
    Ok(())
}

/// commandの標準出力。`sandbox::read`と同じ読み方で、出所だけが違う。
fn read(inside: &dyn Inside, args: &[&str]) -> Result<String> {
    let outcome = inside.exec(args)?.require_success()?;
    Ok(outcome.stdout_text().trim().to_string())
}
