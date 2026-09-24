use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::paths::ProjectParent;
use crate::repository::RepositoryIdentity;

use super::{AddTarget, Args, LocalRepository};

/// `add`の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddRequest {
    /// 登録対象。hostにあるrepositoryのpath、またはclone URLから解釈する。
    pub repository: RepositoryIdentity,
    pub worktrees: Option<u32>,
    pub detach: Option<String>,
    /// attached modeで起点にするbranchが登録時に決まっていれば、その名前。
    ///
    /// hostにあるrepositoryは、そのrepositoryが今いるbranchから始める。GitHubの
    /// repositoryは構築時にremote default branchを解決するため`None`とする。
    pub start_branch: Option<String>,
}

impl AddRequest {
    /// command lineの指定を、登録できる要求にする。
    ///
    /// hostにあるrepositoryは、ここで実在を確かめて正規化する。attached modeで始める
    /// なら、そのrepositoryがbranchの上にいることを求める。
    pub fn resolve(
        args: &Args,
        parent: &ProjectParent,
        host: &dyn HostEnvironment,
    ) -> Result<AddRequest> {
        let (repository, start_branch) = match &args.target {
            AddTarget::Clone(repository) => (repository.clone(), None),
            AddTarget::Local { path, name } => {
                let local = LocalRepository::resolve(host, parent, path, name.as_deref())?;
                match (&args.detach, local.branch) {
                    (Some(_), _) => (local.identity, None),
                    (None, Some(branch)) => (local.identity, Some(branch)),
                    (None, None) => {
                        return Err(Error::single(
                            crate::diagnostics::Diagnostic::new(
                                ErrorId::HostRepositoryDetached,
                                msg!("error-host-repository-detached"),
                            )
                            .remediation(msg!("remediation-host-repository-detached")),
                        ));
                    }
                }
            }
        };
        Ok(AddRequest {
            repository,
            worktrees: args.worktrees,
            detach: args.detach.clone(),
            start_branch,
        })
    }
}

#[cfg(test)]
#[path = "add_request_test.rs"]
mod add_request_test;
