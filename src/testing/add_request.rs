//! workflowへ渡す要求。

use crate::project::ProjectId;
use crate::workflow::add::AddRequest;

/// `add`が受け取る要求。
pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> AddRequest {
    AddRequest {
        project: ProjectId::parse(project).expect("valid project id"),
        worktrees,
        detach: detach.map(|value| value.to_string()),
    }
}
