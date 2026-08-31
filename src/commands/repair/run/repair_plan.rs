use crate::design::Cell;
use crate::support::provisioning::ProvisioningState;

/// `repair`がmutation前に示す固定targetと変更範囲。
#[derive(Debug, Clone)]
pub struct RepairPlan {
    pub project: String,
    pub sandbox: String,
    pub state: ProvisioningState,
    pub target_generation: String,
    pub actions: Vec<Cell>,
}
