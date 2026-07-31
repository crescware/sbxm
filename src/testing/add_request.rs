//! workflowへ渡す要求。

use crate::testing::outcome::{Checked, Required};

use crate::commands::add::run::AddRequest;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;
use crate::testing::project::ssh_repository;

/// `add`が受け取る要求。
pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> Checked<AddRequest> {
    Ok(from(ssh_repository(project)?, worktrees, detach))
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
        detach: detach.map(std::string::ToString::to_string),
    }
}

/// 要求が指す案件ID。
pub fn project_of(request: &AddRequest) -> Checked<ProjectId> {
    ProjectId::parse(&request.repository.display_id())
        .required_because("the request names a project")
}
