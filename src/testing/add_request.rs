//! workflowへ渡す要求。

use crate::commands::add::run::AddRequest;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;
use crate::testing::project::ssh_repository;

/// `add`が受け取る要求。
pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> AddRequest {
    from(ssh_repository(project), worktrees, detach)
}

/// clone URLを明示する要求。
pub fn from(
    repository: RepositoryIdentity,
    worktrees: Option<u32>,
    detach: Option<&str>,
) -> AddRequest {
    AddRequest {
        repository,
        worktrees,
        detach: detach.map(|value| value.to_string()),
    }
}

/// 要求が指す案件ID。
pub fn project_of(request: &AddRequest) -> ProjectId {
    ProjectId::parse(&request.repository.display_id()).expect("the request names a project")
}
