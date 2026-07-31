use super::StopResult;

/// 対象1件の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopOutcome {
    pub project: String,
    pub sandbox: String,
    pub result: StopResult,
}
