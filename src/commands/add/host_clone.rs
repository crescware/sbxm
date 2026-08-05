//! Host clone。
//!
//! 案件directory直下へrepositoryをcloneし、editorとhost上の操作から使えるようにする。
//! 既存cloneは、それが対象repositoryのcloneであると証明できた場合だけ再利用する。
//! dirty状態は構築の継続を妨げない。

use std::fs;
use std::path::{Path, PathBuf};

use crate::command::{CommandSpec, HostEnvironment, TerminalCommand, TimeoutClass};
use crate::design::Fact;
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::repository::{RepositoryIdentity, accepted_clone_url_forms};

/// 採用したhost clone。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostClone {
    pub path: PathBuf,
    /// この実行でcloneしたか。既存cloneを再利用した場合は`false`。
    pub created: bool,
}

impl HostClone {
    /// host cloneを用意する。
    ///
    /// `GitHubへのSSH認証とrepository` accessは、owner、repository、利用者のSSH設定に
    /// よって結果が変わる。この工程のcloneを疎通の正本の検査とする。
    pub fn ensure(
        host: &dyn HostEnvironment,
        paths: &ProjectPaths,
        repository: &RepositoryIdentity,
        progress: &mut dyn ProgressSink,
    ) -> Result<HostClone> {
        let target = paths.host_clone();

        // 不在と、観測できないことを区別する。そこにあるものを確かめられないまま
        // cloneへ進むと、既存の成果物の上で外部commandの挙動に判断を委ねることになる。
        match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                clone(host, &target, repository, progress)?;
                // 作成直後も、再利用と同じ規則で成果物を検証する。
                inspect(host, paths, repository, &target)?;
                Ok(HostClone {
                    path: target,
                    created: true,
                })
            }
            Err(error) => Err(PathScope::ProjectPath.unreadable_error(&target, &error.to_string())),
            Ok(_) => {
                inspect(host, paths, repository, &target)?;
                Ok(HostClone {
                    path: target,
                    created: false,
                })
            }
        }
    }
}

fn clone(
    host: &dyn HostEnvironment,
    target: &Path,
    repository: &RepositoryIdentity,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-cloning-host"));
    // 進捗はgitが出したまま転送する。sbxmは実況を重ねない。clone URLはshellを介さず、
    // validation済みのargumentとして渡す。
    let command = TerminalCommand::relayed(
        "git",
        &[
            "clone",
            // pipe越しでもgitが進捗を出すよう明示する。
            "--progress",
            repository.clone_url(),
            &paths::display(target),
        ],
    )
    .timeout(TimeoutClass::RepositoryTransfer);
    host.run_with_terminal(&command, progress)?
        .require_success()?;
    Ok(())
}

/// 既存cloneが対象repositoryのものであることを確認する。
fn inspect(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    repository: &RepositoryIdentity,
    target: &Path,
) -> Result<()> {
    if paths::is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(target)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(target, &error.to_string()))?;
    if !metadata.is_dir() {
        return Err(unusable(target, target, msg!("cause-not-a-directory")));
    }
    require_git_dir_inside_the_project(paths, target)?;

    if read_git(host, target, &["rev-parse", "--is-bare-repository"])? != "false" {
        return Err(unusable(target, target, msg!("cause-bare-repository")));
    }

    let top_level = read_git(host, target, &["rev-parse", "--show-toplevel"])?;
    let observed = paths::real_path(Path::new(&top_level));
    let expected = paths::real_path(target);
    if observed != expected {
        return Err(unusable(
            target,
            target,
            msg!(
                "cause-working-tree-elsewhere",
                expected = paths::display(&expected),
                observed = paths::display(&observed)
            ),
        ));
    }

    let urls = read_git(host, target, &["config", "--get-all", "remote.origin.url"])?;
    let urls: Vec<&str> = urls
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let [url] = urls.as_slice() else {
        return Err(unusable(
            target,
            target,
            msg!("cause-origin-ambiguous", count = urls.len()),
        ));
    };
    // originも登録時と同じ規則でparseする。読めないURLを一致しているとみなさない。
    let Ok(observed) = RepositoryIdentity::parse_clone_url(url) else {
        return Err(unusable(
            target,
            target,
            msg!(
                "cause-clone-url-unrecognized",
                observed = url,
                accepted = accepted_clone_url_forms()
            ),
        ));
    };
    if !observed.same_target(repository) {
        return Err(unusable(
            target,
            target,
            msg!(
                "cause-origin-elsewhere",
                observed = observed,
                declared = repository.clone_url()
            ),
        ));
    }
    Ok(())
}

/// `.git`が案件directoryの外を指していないことを確認する。
///
/// worktree fileは`gitdir: <path>`の1行を持つ。指す先がproject root外にある場合、
/// 案件の成果物としては扱えない。
fn require_git_dir_inside_the_project(paths: &ProjectPaths, target: &Path) -> Result<()> {
    let git_path = target.join(".git");
    // `.git`側の不備は`.git`を示す。cloneのrootを示しても確かめる先が分からない。
    let unreadable = |error: &std::io::Error| {
        Error::single(
            Diagnostic::new(
                ErrorId::HostCloneUnusable,
                msg!("error-host-clone-unusable"),
            )
            .fact(Fact::path(&paths::display(&git_path)))
            .fact(Fact::cause(&error.to_string()))
            .remediation(msg!(
                "remediation-host-clone-unusable",
                path = paths::display(target)
            )),
        )
    };
    let metadata = fs::symlink_metadata(&git_path).map_err(|error| unreadable(&error))?;

    if metadata.is_dir() {
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(unusable(
            target,
            &git_path,
            msg!("cause-not-a-regular-file"),
        ));
    }

    let contents = fs::read_to_string(&git_path).map_err(|error| unreadable(&error))?;
    let Some(declared) = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))
    else {
        return Err(unusable(
            target,
            &git_path,
            msg!("cause-no-git-directory-named"),
        ));
    };
    let declared = declared.trim();
    let resolved = if Path::new(declared).is_absolute() {
        paths::lexically_standardize(Path::new(declared))
    } else {
        paths::lexically_standardize(&target.join(declared))
    };
    if !resolved.starts_with(paths.root()) {
        return Err(unusable(
            target,
            &git_path,
            msg!(
                "cause-git-directory-outside",
                observed = paths::display(&resolved),
                root = paths::display(paths.root())
            ),
        ));
    }
    Ok(())
}

/// 作業directoryを対象cloneに固定してgitの結果を読む。
fn read_git(host: &dyn HostEnvironment, target: &Path, args: &[&str]) -> Result<String> {
    let spec = CommandSpec::capture("git", args)
        .timeout(TimeoutClass::LocalFilesystem)
        .working_dir(target);
    let outcome = host.run(&spec)?.require_success()?;
    Ok(outcome.stdout_text().trim().to_string())
}

/// 既存cloneを自動削除せず、観測した不一致を示して停止する。
///
/// `shown`は読み手が見に行くべきpathであり、`target`は対処が指すcloneそのものである。
/// `.git`の不備は`.git`を示したほうが確かめやすい。
fn unusable(target: &Path, shown: &Path, reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::HostCloneUnusable,
            msg!("error-host-clone-unusable"),
        )
        .fact(Fact::path(&paths::display(shown)))
        .fact(Fact::reason(reason))
        .remediation(msg!(
            "remediation-host-clone-unusable",
            path = paths::display(target)
        )),
    )
}

#[cfg(test)]
#[path = "host_clone_test.rs"]
mod host_clone_test;
