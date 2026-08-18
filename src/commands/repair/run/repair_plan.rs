use crate::design::Warning;
use crate::paths::ExclusiveLock;
use crate::support::select::Locked;

/// 明示的repairで変更する対象を固定した計画。
pub struct RepairPlan {
    pub project: String,
    pub sandbox: String,
    pub target_generation: String,
    pub artifacts: Vec<String>,
    pub(crate) warnings: Vec<Warning>,
    pub(crate) locked: Locked,
    pub(crate) session_lease: ExclusiveLock,
}
