use crate::support::inventory::ProjectState;
use crate::support::protection::{ConfirmableLoss, WorktreeReport};

use super::Target;

/// 削除前に見せる内容。
#[derive(Debug, Clone)]
pub struct DestroyPlan {
    pub project: String,
    pub sandbox: String,
    pub state: ProjectState,
    /// 中を観測するために、停止していたSandboxをこの実行が起動したか。
    ///
    /// 起動は計画を作るために行い、確認をcancelしても元へ戻さない。黙って残さないよう、
    /// 計画と一緒にその事実を告げる。
    pub started: bool,
    /// データ保護検査とactive session検査を省略するか。
    pub force: bool,
    /// 通常modeで観測したworktree。force modeでは空。
    pub worktrees: Vec<WorktreeReport>,
    /// 確認すれば削除してよい対象。force modeでは空。
    pub confirmable_losses: Vec<ConfirmableLoss>,
    pub removes: Vec<Target>,
    pub keeps: Vec<Target>,
    /// 再登録に使うcommand。
    pub re_register: String,
}
