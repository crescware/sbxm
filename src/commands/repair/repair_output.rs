use crate::design::Warning;

/// `repair`の実行結果。
#[derive(Debug, Clone)]
pub struct RepairOutput {
    pub project: String,
    pub sandbox: String,
    pub target_generation: String,
    pub changed: bool,
    pub warnings: Vec<Warning>,
}
