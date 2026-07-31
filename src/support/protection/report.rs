use super::WorktreeReport;

/// 検査結果。
#[derive(Debug, Clone)]
pub struct Report {
    pub worktrees: Vec<WorktreeReport>,
}
