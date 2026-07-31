use crate::testing::outcome::{Checked, Required};

use crate::project::ProjectId;

/// testが書く案件IDは常に妥当とする。
pub fn project_id(value: &str) -> Checked<ProjectId> {
    ProjectId::parse(value).required_because("valid project id")
}
