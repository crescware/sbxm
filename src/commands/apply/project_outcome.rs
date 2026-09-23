use super::ProjectResult;

/// `apply --files --all`での対象1件の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectOutcome {
    pub project: String,
    pub sandbox: String,
    pub result: ProjectResult,
}
