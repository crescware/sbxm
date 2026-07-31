use crate::metadata::CreationMode;

use crate::design::Warning;
use crate::support::files::PlacedFile;
use crate::support::tools::Note;

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
    /// Sandboxに入っているtoolが返した案内。sbxmが代わりに実行しないことを示す。
    pub notes: Vec<Note>,
    /// 既に構築済みで、この実行が何も変更しなかったか。
    pub already_built: bool,
    pub warnings: Vec<Warning>,
}
