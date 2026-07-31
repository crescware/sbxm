use crate::metadata::GitIdentity;
use crate::repository::RepositoryIdentity;

/// `add`の目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub repository: RepositoryIdentity,
    pub worktrees: Option<u32>,
    pub detach: Option<String>,
    /// command lineで宣言された名義。宣言が無ければ`None`。
    pub git_identity: Option<GitIdentity>,
}
