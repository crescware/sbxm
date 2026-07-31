use crate::testing::outcome::Checked;

use crate::commands::add::AddRequest;
use crate::testing::project::ssh_repository;

use super::from;

/// `add`が受け取る要求。
pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> Checked<AddRequest> {
    Ok(from(ssh_repository(project)?, worktrees, detach))
}
