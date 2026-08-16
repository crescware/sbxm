use super::ListState;

/// 一覧の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project: String,
    /// registryが指すhost project root。
    pub root: String,
    pub sandbox: String,
    /// runtimeとhostの観測結果から、利用者が次に取る行動へ写した状態。
    pub state: ListState,
}
