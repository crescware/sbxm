use crate::repository::RepositoryIdentity;

/// `add`の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddRequest {
    /// clone URLから解釈した登録対象。
    pub repository: RepositoryIdentity,
    pub worktrees: Option<u32>,
    pub detach: Option<String>,
}
