use crate::paths::ProjectPaths;
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
