use crate::paths::{ExclusiveLock, ProjectPaths};
use crate::project::SandboxName;

use crate::support::inventory::ProjectState;
use crate::support::select::{self};

use super::{DestroyPlan, Unregistration};

/// lockを保持したまま確認を挟むための状態。
#[derive(Debug)]
pub struct Prepared {
    pub plan: DestroyPlan,
    pub(super) paths: ProjectPaths,
    pub(super) name: SandboxName,
    pub(super) state: ProjectState,
    pub(super) force: bool,
    pub(super) locked: select::Locked,
    /// 最終protection inspectからsandbox remove完了まで保持するexclusive session lease。
    ///
    /// `--force`は保護ゲートと同様にこのleaseも意図的に迂回するため`None`になる。
    /// 読まれることはなく、`Prepared`が破棄されるときのdropだけが意味を持つ。
    pub(super) _session_lease: Option<ExclusiveLock>,
}

impl Prepared {
    /// 管理解除後にregistryから外す案件。
    pub fn unregistration(&self) -> Unregistration {
        Unregistration {
            paths: self.paths.clone(),
            repository: self.locked.metadata.repository.clone(),
        }
    }
}
