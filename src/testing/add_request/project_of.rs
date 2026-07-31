use crate::testing::outcome::{Checked, Required};

use crate::commands::add::AddRequest;
use crate::project::ProjectId;

/// 要求が指す案件ID。
pub fn project_of(request: &AddRequest) -> Checked<ProjectId> {
    ProjectId::parse(&request.repository.display_id())
        .required_because("the request names a project")
}
