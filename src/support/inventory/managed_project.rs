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
    /// 初回構築のintentが残っている。一覧が案件ごとにSandboxの中まで観測することは
    /// ないため、metadataだけで確実に言えるこの1点を、`open`が進めない印として持つ。
    pub recovery_pending: bool,
}
