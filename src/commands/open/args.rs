use crate::project::ProjectId;

/// `open`の対象と、接続開始directory。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub project: Option<ProjectId>,
    /// 0-basedのmanaged worktree index。省略時はrepository rootへ入る。
    pub index: Option<u32>,
}
