use crate::metadata::CreationMode;

use crate::design::Warning;
use crate::support::files::PlacedFile;

use super::WorktreeRow;

/// `prepare`の結果。
#[derive(Debug, Clone)]
pub struct PrepareOutput {
    pub project: String,
    pub sandbox: String,
    pub mode: CreationMode,
    pub start_ref: String,
    pub sandbox_state: crate::compatibility::SandboxState,
    pub worktrees: Vec<WorktreeRow>,
    pub files: Vec<PlacedFile>,
    /// 既に構築済みで、この実行が何も変更しなかったか。
    pub already_built: bool,
    pub warnings: Vec<Warning>,
}
