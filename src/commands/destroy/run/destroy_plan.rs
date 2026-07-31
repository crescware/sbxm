use crate::support::inventory::ProjectState;
use crate::support::protection::WorktreeReport;

use super::Target;

/// 削除前に見せる内容。
#[derive(Debug, Clone)]
pub struct DestroyPlan {
    pub project: String,
    pub sandbox: String,
    pub state: ProjectState,
    /// データ保護検査とactive session検査を省略するか。
    pub force: bool,
    /// 通常modeで観測したworktree。force modeでは空。
    pub worktrees: Vec<WorktreeReport>,
    pub removes: Vec<Target>,
    pub keeps: Vec<Target>,
    /// 再登録に使うcommand。
    pub re_register: String,
}
