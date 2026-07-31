use std::path::PathBuf;

use super::Observed;

/// 1件の管理案件と、その現在の状態。
#[derive(Debug, Clone)]
pub struct ManagedProject {
    /// 表示に使う`<owner>/<repository>`。registry entryから決まる。
    pub display_id: String,
    pub project_root: PathBuf,
    pub sandbox: String,
    pub observed: Observed,
}
