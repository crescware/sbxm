use crate::project::ProjectId;

/// `apply`の引数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub project: Option<ProjectId>,
    /// 登録済みのすべての案件へ宣言fileを配置する。
    pub all: bool,
    /// global configが宣言するfileを再配置する。
    pub files: bool,
    /// Sandboxの中で変更された宣言fileも置き換える。
    pub force: bool,
    /// managed worktreeの目標本数。
    pub worktrees: Option<u32>,
}
