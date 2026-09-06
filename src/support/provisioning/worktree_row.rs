use crate::metadata::CreationMode;

/// provisioning結果のmanaged worktree 1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    pub created_from: String,
    /// 観測したHEAD。読めない、または空の応答は観測不能として拒否するため、この行が
    /// 作られる時点で必ず値を持つ。
    pub head: String,
    pub mode: CreationMode,
}
