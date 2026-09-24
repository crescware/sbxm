use crate::design::Warning;

/// `rebuild`の結果。
#[derive(Debug, Clone)]
pub struct RebuildOutput {
    pub project: String,
    pub sandbox: String,
    /// 適用済みになったDockerfile hash。
    pub applied: String,
    /// hostへ保存してあったため、作り直したSandboxへ戻したbranch。
    pub restored: Vec<String>,
    pub warnings: Vec<Warning>,
}
