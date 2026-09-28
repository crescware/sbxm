use std::path::{Path, PathBuf};

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{ErrorId, Result};
use crate::support::repository::{host_git, older_git};
use crate::support::worktree;

use super::ReflectResult;

/// hostでcheckoutしている`reference`を、そのworktreeで`source`の先端まで早送りする。
///
/// 利用者がそのworktreeで`git merge --ff-only`したときと同じく、進めてよいかはgitが
/// 決める。重ならない未commitの変更は残したまま進み、重なる変更があればgitが断る。
/// hostのrepositoryの`receive.denyCurrentBranch`は見ない。`sync`を実行する利用者は
/// hostにいて、hostが動くことを承知している。
///
/// hostのrepositoryの`pre-receive` hookは、その前のpushでこのbranchも見ており、断れば
/// ここへは来ない。`update`と`post-receive`はこのbranchには走らず、手でmergeしたときと
/// 同じく`post-merge`が走る。
///
/// 早送りであることは、呼び出し側のpushが確かめてある。gitは早送りでない更新を、
/// checkoutしているかを問う前に断る。
///
/// branchを指しているworktreeが1つに決まらなければ動かさない。rebaseやbisectの途中の
/// worktreeは、branchを指さないが、gitはそのbranchをcheckout中として扱う。
pub(super) fn move_checked_out(
    host: &dyn HostEnvironment,
    repository: &Path,
    source: &str,
    reference: &str,
) -> Result<ReflectResult> {
    let Some(worktree) = checked_out_in(host, repository, reference)? else {
        return Ok(ReflectResult::CheckedOut);
    };
    let merged = match host_git(
        host,
        &worktree,
        // `merge.autoStash`があっても、変更を退避して戻す形にはしない。戻すときに競合
        // しうる。
        &["merge", "--ff-only", "--no-autostash", "--quiet", source],
        None,
        // 作業treeのfileを書き換え、hostのrepositoryの`post-merge` hookも走る。
        TimeoutClass::RepositoryTransfer,
    ) {
        Ok(merged) => merged,
        // lockしたworktreeは、directoryが無くても片付けてよいとは見なされない。
        Err(error) if error.contains_id(ErrorId::HostRepositoryMissing) => {
            return Ok(ReflectResult::CheckedOut);
        }
        Err(error) => return Err(error),
    };
    if merged.success() {
        return Ok(ReflectResult::Updated);
    }
    let paths = overlapping(host, &worktree, source)?;
    if paths.is_empty() {
        // 重なる変更のほかに、mergeの途中であることなどでもgitは断る。gitの答えを示す。
        let stderr = String::from_utf8_lossy(&merged.stderr);
        let reason = stderr
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default();
        return Ok(ReflectResult::Refused {
            reason: reason.to_string(),
        });
    }
    Ok(ReflectResult::LocalChanges { worktree, paths })
}

/// `reference`を指しているhostのworktree。1つに決まらなければ`None`。
fn checked_out_in(
    host: &dyn HostEnvironment,
    repository: &Path,
    reference: &str,
) -> Result<Option<PathBuf>> {
    let listed = host_git(
        host,
        repository,
        &["worktree", "list", "--porcelain", "-z"],
        None,
        TimeoutClass::LocalFilesystem,
    )?;
    // `-z`はgit 2.36からである。失敗したときだけ、古いgitのためかを確かめる。
    let listed = match listed.require_success() {
        Ok(listed) => listed,
        Err(error) => return Err(older_git(host, repository).unwrap_or(error)),
    };
    let mut found = worktree::parse_list(&listed.stdout_text())?
        .into_iter()
        .filter(|entry| !entry.prunable && entry.branch.as_deref() == Some(reference));
    Ok(match (found.next(), found.next()) {
        (Some(entry), None) => Some(PathBuf::from(entry.path)),
        _ => None,
    })
}

/// `worktree`の未commitの変更のうち、`source`までの早送りも変えるfile。
///
/// gitが断った理由を示すためだけに使う。進めてよいかはgitが決める。
fn overlapping(host: &dyn HostEnvironment, worktree: &Path, source: &str) -> Result<Vec<String>> {
    let incoming = names(
        host,
        worktree,
        &["diff-tree", "-r", "--name-only", "-z", "HEAD", source],
    )?;
    let mut local = names(
        host,
        worktree,
        &["diff", "--no-renames", "--name-only", "-z", "HEAD"],
    )?;
    local.extend(names(
        host,
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?);
    Ok(incoming
        .into_iter()
        .filter(|path| local.contains(path))
        .collect())
}

/// pathの一覧を`NUL`で区切って出すgitを走らせる。
fn names(host: &dyn HostEnvironment, worktree: &Path, args: &[&str]) -> Result<Vec<String>> {
    let listed =
        host_git(host, worktree, args, None, TimeoutClass::LocalFilesystem)?.require_success()?;
    Ok(listed
        .stdout_text()
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect())
}
