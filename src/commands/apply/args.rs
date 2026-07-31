use crate::project::ProjectId;

/// `apply`の引数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub project: ProjectId,
    /// global configが宣言するfileを再配置する。
    pub files: bool,
    /// managed worktreeの目標本数。
    pub worktrees: Option<u32>,
}
