use std::path::PathBuf;

use super::{Observed, WorkspaceState};

/// 1件の管理案件と、その現在の状態。
#[derive(Debug, Clone)]
pub struct ManagedProject {
    /// 表示に使う`<owner>/<repository>`。registry entryから決まる。
    pub display_id: String,
    pub project_root: PathBuf,
    pub sandbox: String,
    pub observed: Observed,
    /// 中立workspace directoryの実在。`observed`とは別の事実である。
    pub workspace: WorkspaceState,
}
