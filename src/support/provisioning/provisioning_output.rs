use crate::compatibility::SandboxState;
use crate::design::Warning;
use crate::metadata::CreationMode;

use super::WorktreeRow;

/// 初回構築の結果。prepareとrepairの表示は同じ結果を共有する。
#[derive(Debug, Clone)]
pub struct ProvisioningOutput {
    pub project: String,
    pub sandbox: String,
    pub mode: CreationMode,
    pub start_ref: String,
    pub sandbox_state: SandboxState,
    pub worktrees: Vec<WorktreeRow>,
    pub files: Vec<crate::support::files::PlacedFile>,
    /// 既に構築済みで、この実行が何も変更しなかったか。
    pub already_built: bool,
    pub warnings: Vec<Warning>,
}
