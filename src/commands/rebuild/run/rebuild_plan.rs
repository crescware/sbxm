use crate::support::protection::ConfirmableLoss;

/// 再構築前に見せる内容。
#[derive(Debug, Clone)]
pub struct RebuildPlan {
    pub project: String,
    pub sandbox: String,
    /// 現在のDockerfileのhash。
    pub current_generation: String,
    /// 適用する世代のhash。resume中は固定済みの値である。
    pub target_generation: String,
    /// 確認すれば作り直してよい対象。
    pub confirmable_losses: Vec<ConfirmableLoss>,
}
