//! hostにあるrepository。
//!
//! `sbxm add --local`は利用者のrepositoryをcloneせず、その場所のまま登録する。
//! repositoryそのものがoriginの役を持つため、渡すのはgit directoryである。`.git`、
//! bare repository、worktreeやsubmoduleの`.git` fileを受け付ける。working treeの
//! directoryは受け付けない。directoryの中にはrepositoryがいくつあってもよく、どれを
//! 指したのかが決まらないためである。

use std::fs;
use std::path::Path;

use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, ProjectParent};
use crate::repository::{RepositoryIdentity, clone_directory_name};

/// 確かめたhostのrepository。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRepository {
    pub identity: RepositoryIdentity,
    /// 渡されたgit directoryが今いるbranch。detachedなら`None`。
    ///
    /// worktreeの`.git` fileを渡されたら、そのworktreeのbranchである。
    pub branch: Option<String>,
    /// 案件directoryを作る`parent`が、このrepositoryのworking treeかgit directoryの
    /// 中にあるか。
    pub encloses_parent: bool,
}

impl LocalRepository {
    /// `path`を実在するpathへ解決し、登録できるrepositoryであることを確かめる。
    ///
    /// 相対pathはcwdから解決する。symlinkと`.git` fileは、それが指すrepositoryを
    /// 登録する。案件の名前を省略すれば、`git clone`が作るdirectoryの名前を使う。
    pub fn resolve(
        host: &dyn HostEnvironment,
        parent: &ProjectParent,
        path: &Path,
        name: Option<&str>,
    ) -> Result<LocalRepository> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            parent.as_path().join(path)
        };
        let real = fs::canonicalize(&absolute)
            .map_err(|error| unusable(&absolute, Fact::cause(&error.to_string())))?;
        let git_dir = utf8(&real)?;
        // worktreeの`.git`は、そのworktreeだけのdirectoryを指す。記録するのは、
        // worktreeが共有するrepositoryの本体である。
        let repository = paths::real_path(Path::new(&read_git(
            host,
            git_dir,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?));
        let identity = identity(utf8(&repository)?, name)?;
        Ok(LocalRepository {
            identity,
            branch: current_branch(host, git_dir)?,
            encloses_parent: encloses(host, &repository, parent)?,
        })
    }
}

/// 名前を省略した案件は、`git clone`が作るdirectoryの名前が案件の名前として使えなければ
/// `--name`を求める。
fn identity(path: &str, name: Option<&str>) -> Result<RepositoryIdentity> {
    if let Some(name) = name {
        return RepositoryIdentity::local(path, name);
    }
    let directory = clone_directory_name(path).unwrap_or_default();
    RepositoryIdentity::local(path, directory).map_err(|_| {
        Error::single(
            Diagnostic::new(
                ErrorId::LocalNameUnusable,
                msg!("error-local-name-unusable", name = directory),
            )
            .remediation(msg!("remediation-local-name-unusable")),
        )
    })
}

/// branchの上にいれば、その名前。
///
/// 完全なref名から`refs/heads/`を外して得る。`--short`は、同じ名前のtagがあると
/// `heads/main`のように曖昧さを避けた名前を返す。
fn current_branch(host: &dyn HostEnvironment, git_dir: &str) -> Result<Option<String>> {
    let outcome = host.run(&git(git_dir, &["symbolic-ref", "--quiet", "HEAD"]))?;
    // detached HEADは終了status 1で答える。それ以外の失敗はgitの失敗として伝える。
    if outcome.status.code() == Some(1) {
        return Ok(None);
    }
    let reference = outcome.require_success()?.stdout_text().trim().to_string();
    Ok(Some(
        reference
            .strip_prefix("refs/heads/")
            .map_or(reference.clone(), str::to_string),
    ))
}

/// `parent`が、`repository`のworking treeかgit directoryの中にあるか。
///
/// working treeを数え上げず、`parent`から上へgitに探させ、見つかったrepositoryと比べる。
/// `--separate-git-dir`で分けたrepositoryのように、gitはgit directoryからworking treeを
/// たどれないことがある。repositoryが見つからなければ、中にはない。`parent`の中に
/// 別のrepositoryがあれば、それはこのrepositoryではない。
fn encloses(host: &dyn HostEnvironment, repository: &Path, parent: &ProjectParent) -> Result<bool> {
    let directory = parent.as_path().to_string_lossy();
    let spec = CommandSpec::capture(
        "git",
        &[
            "-C",
            directory.as_ref(),
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ],
    )
    .env(EnvPolicy::HostRepository)
    .timeout(TimeoutClass::LocalFilesystem);
    let outcome = host.run(&spec)?;
    if !outcome.success() {
        return Ok(false);
    }
    Ok(paths::real_path(Path::new(outcome.stdout_text().trim())) == repository)
}

/// `git_dir`でgitの結果を読む。
fn read_git(host: &dyn HostEnvironment, git_dir: &str, args: &[&str]) -> Result<String> {
    let outcome = host.run(&git(git_dir, args))?;
    if !outcome.success() {
        // gitが読めないpathは、git directoryではないものとして理由ごと示す。
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        return Err(unusable(Path::new(git_dir), Fact::cause(stderr.trim())));
    }
    Ok(outcome.stdout_text().trim().to_string())
}

/// `git_dir`をgit directoryとして読むgit。
///
/// 登録したrepositoryを読む`host_git`と同じく、呼び出し元が設定したrepositoryの場所を
/// 引き継がない。`--git-dir`で渡すため、gitは`git_dir`の上も中も探さない。
fn git(git_dir: &str, args: &[&str]) -> CommandSpec {
    let mut full = vec!["--git-dir", git_dir];
    full.extend_from_slice(args);
    CommandSpec::capture("git", &full)
        .env(EnvPolicy::HostRepository)
        .timeout(TimeoutClass::LocalFilesystem)
}

/// 記録し、gitへ渡せるpath。
fn utf8(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| unusable(path, Fact::reason(msg!("cause-path-not-utf8"))))
}

fn unusable(path: &Path, reason: Fact) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::LocalRepositoryUnusable,
            msg!("error-local-repository-unusable"),
        )
        .fact(Fact::path(&paths::display(path)))
        .fact(reason)
        .remediation(msg!("remediation-local-repository-unusable")),
    )
}

#[cfg(test)]
#[path = "local_repository_test.rs"]
mod local_repository_test;
