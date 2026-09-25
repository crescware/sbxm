use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
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
    /// 案件directoryを作るdirectoryが、hostにある登録対象のrepositoryの中にあるか。
    ///
    /// working treeやgit directoryの中には、新しい案件directoryを作らせない。GitHubの
    /// repositoryでは`false`とする。
    pub parent_inside_repository: bool,
}

impl AddRequest {
    /// command lineの指定を、登録できる要求にする。
    ///
    /// hostにあるrepositoryは、ここで実在を確かめて正規化する。branchの上にいれば、
    /// attached modeの起点としてそのbranchを持つ。detachedでも、ここでは断らない。
    /// 登録済みの案件は、保存済みの起点で続けられるためである。`parent`がrepositoryの
    /// 中にあることも同じ理由で断らず、新しく記録するときに断る。
    pub fn resolve(
        args: &Args,
        parent: &ProjectParent,
        host: &dyn HostEnvironment,
    ) -> Result<AddRequest> {
        let (repository, start_branch, parent_inside_repository) = match &args.target {
            AddTarget::Clone(repository) => (repository.clone(), None, false),
            AddTarget::Local { path, name } => {
                let local = LocalRepository::resolve(host, parent, path, name.as_deref())?;
                let start_branch = if args.detach.is_some() {
                    None
                } else {
                    local.branch
                };
                (local.identity, start_branch, local.encloses_parent)
            }
        };
        Ok(AddRequest {
            repository,
            worktrees: args.worktrees,
            detach: args.detach.clone(),
            start_branch,
            parent_inside_repository,
        })
    }
}

#[cfg(test)]
#[path = "add_request_test.rs"]
mod add_request_test;
