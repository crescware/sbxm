use crate::boundary::host::protocol::SandboxEntry;

use super::ManagedProject;

/// 管理案件と、管理外のSandbox。
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// canonical ID byte昇順。
    pub projects: Vec<ManagedProject>,
    /// Sandbox name byte昇順。
    pub unmanaged: Vec<SandboxEntry>,
}

impl Snapshot {
    /// 全案件が登録済みで、entryと成果物が一致しているか。
    pub fn is_settled(&self) -> bool {
        self.projects
            .iter()
            .all(|project| project.observed.is_settled())
    }
}
