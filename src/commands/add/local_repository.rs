//! hostにあるrepository。
//!
//! `sbxm add --local`は利用者のrepositoryをcloneせず、その場所のまま登録する。hostの
//! `.git`がoriginの役を持つため、登録する前に、それがworking treeの最上位であることを
//! 確かめる。

use std::fs;
use std::path::{Path, PathBuf};

use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, ProjectParent};
use crate::repository::RepositoryIdentity;

/// 確かめたhostのrepository。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRepository {
    pub identity: RepositoryIdentity,
    /// hostのrepositoryが今いるbranch。detachedなら`None`。
    pub branch: Option<String>,
}

impl LocalRepository {
    /// `path`を実在するpathへ解決し、登録できるrepositoryであることを確かめる。
    ///
    /// 相対pathはcwdから解決する。symlinkは解決したpathを登録する。案件の名前を省略すれば
    /// directory名を使う。
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
        if !real.is_dir() {
            return Err(unusable(&real, Fact::reason(msg!("cause-not-a-directory"))));
        }
        if read_git(host, &real, &["rev-parse", "--is-bare-repository"])? != "false" {
            return Err(unusable(&real, Fact::reason(msg!("cause-bare-repository"))));
        }
        let top_level = PathBuf::from(read_git(host, &real, &["rev-parse", "--show-toplevel"])?);
        if paths::real_path(&top_level) != real {
            return Err(unusable(
                &real,
                Fact::reason(msg!(
                    "cause-working-tree-elsewhere",
                    expected = paths::display(&real),
                    observed = paths::display(&top_level)
                )),
            ));
        }
        let Some(text) = real.to_str() else {
            return Err(unusable(&real, Fact::reason(msg!("cause-path-not-utf8"))));
        };
        let identity = identity(text, name)?;
        Ok(LocalRepository {
            identity,
            branch: current_branch(host, &real)?,
        })
    }
}

/// 名前を省略した案件は、directory名が案件の名前として使えなければ`--name`を求める。
fn identity(path: &str, name: Option<&str>) -> Result<RepositoryIdentity> {
    if let Some(name) = name {
        return RepositoryIdentity::local(path, name);
    }
    let directory = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
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
fn current_branch(host: &dyn HostEnvironment, repository: &Path) -> Result<Option<String>> {
    let outcome = host.run(&git(repository, &["symbolic-ref", "--quiet", "HEAD"]))?;
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

/// `repository`でgitの結果を読む。
fn read_git(host: &dyn HostEnvironment, repository: &Path, args: &[&str]) -> Result<String> {
    let outcome = host.run(&git(repository, args))?;
    if !outcome.success() {
        // gitが読めないpathは、repositoryではないものとして理由ごと示す。
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        return Err(unusable(repository, Fact::cause(stderr.trim())));
    }
    Ok(outcome.stdout_text().trim().to_string())
}

/// `repository`で走らせるgit。
///
/// 登録したrepositoryを読む`host_git`と同じく、呼び出し元が設定したrepositoryの場所を
/// 引き継がない。ただし上のdirectoryのrepositoryは探させる。working treeの途中を
/// 指されたとき、その最上位を示して断るためである。作業directoryを固定せず`-C`で渡す。
fn git(repository: &Path, args: &[&str]) -> CommandSpec {
    let directory = repository.to_string_lossy();
    let mut full = vec!["-C", directory.as_ref()];
    full.extend_from_slice(args);
    CommandSpec::capture("git", &full)
        .env(EnvPolicy::HostRepository)
        .timeout(TimeoutClass::LocalFilesystem)
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
