use crate::commands::add::AddRequest;
use crate::repository::RepositoryIdentity;

/// clone URLを明示する要求。
pub fn from(
    repository: RepositoryIdentity,
    worktrees: Option<u32>,
    detach: Option<&str>,
) -> AddRequest {
    AddRequest {
        repository,
        worktrees,
        detach: detach.map(std::string::ToString::to_string),
    }
}
