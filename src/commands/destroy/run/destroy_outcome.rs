use crate::design::Warning;

/// 削除の結果。
#[derive(Debug, Clone)]
pub struct DestroyOutcome {
    pub project: String,
    pub re_register: String,
    pub warnings: Vec<Warning>,
}
