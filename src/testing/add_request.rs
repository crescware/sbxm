//! workflowへ渡す要求。

use crate::testing::project::project_id;
use crate::workflow::add::AddRequest;

/// `add`が受け取る要求。
pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> AddRequest {
    AddRequest {
        project: project_id(project),
        worktrees,
        detach: detach.map(|value| value.to_string()),
    }
}
