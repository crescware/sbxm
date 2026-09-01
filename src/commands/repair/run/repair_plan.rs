use crate::design::Field;
use crate::support::provisioning::ProvisioningState;

use super::RepairAction;

/// `repair`がmutation前に示す、artifact単位の観測事実と実行対象。
#[derive(Debug, Clone)]
pub struct RepairPlan {
    pub project: String,
    pub sandbox: String,
    pub state: ProvisioningState,
    pub target_generation: String,
    /// artifactごとの観測結果。表示専用であり、`execute`はこれを読まない。
    pub observations: Vec<Field>,
    /// 実際に変更する対象。`execute`は表示済みのこの一覧としか一致しない実行を許さない。
    pub actions: Vec<RepairAction>,
}
