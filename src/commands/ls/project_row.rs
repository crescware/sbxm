use crate::support::inventory::Observed;

/// 一覧の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project: String,
    /// registryが指すhost project root。
    pub root: String,
    pub sandbox: String,
    pub observed: Observed,
}
