use crate::support::inventory::{Observed, WorkspaceState};

/// 一覧の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project: String,
    /// registryが指すhost project root。
    pub root: String,
    pub sandbox: String,
    pub observed: Observed,
    /// 中立workspace directoryの実在。`observed`が示すruntime stateとは別の事実である。
    pub workspace: WorkspaceState,
}
