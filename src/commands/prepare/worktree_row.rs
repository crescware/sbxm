use crate::metadata::CreationMode;

/// 出力のworktree 1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    pub created_from: String,
    /// 観測できたHEAD。停止中のSandboxでは読めないため`None`になる。
    pub head: Option<String>,
    pub mode: CreationMode,
}
