//! hostにあるrepository。
//!
//! `sbxm add --local`は利用者のrepositoryをcloneせず、その場所のまま登録する。hostの
//! `.git`がoriginの役を持つため、登録する前に、それがworking treeの最上位であることを
//! 確かめる。

use std::fs;
use std::path::{Path, PathBuf};

use crate::boundary::host::{CommandSpec, HostEnvironment, TimeoutClass};
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
    let spec = CommandSpec::capture("git", &["symbolic-ref", "--quiet", "HEAD"])
        .timeout(TimeoutClass::LocalFilesystem)
        .working_dir(repository);
    let outcome = host.run(&spec)?;
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

/// 作業directoryを`repository`に固定してgitの結果を読む。
fn read_git(host: &dyn HostEnvironment, repository: &Path, args: &[&str]) -> Result<String> {
    let spec = CommandSpec::capture("git", args)
        .timeout(TimeoutClass::LocalFilesystem)
        .working_dir(repository);
    let outcome = host.run(&spec)?;
    if !outcome.success() {
        // gitが読めないpathは、repositoryではないものとして理由ごと示す。
        let stderr = String::from_utf8_lossy(&outcome.stderr);
        return Err(unusable(repository, Fact::cause(stderr.trim())));
    }
    Ok(outcome.stdout_text().trim().to_string())
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
