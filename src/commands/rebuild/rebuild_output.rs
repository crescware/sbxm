use crate::design::Warning;

/// `rebuild`の結果。
#[derive(Debug, Clone)]
pub struct RebuildOutput {
    pub project: String,
    pub sandbox: String,
    /// 適用済みになったDockerfile hash。
    pub applied: String,
    pub warnings: Vec<Warning>,
}
