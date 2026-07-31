use crate::testing::outcome::{Checked, Required};

use crate::project::ProjectId;

pub fn project() -> Checked<ProjectId> {
    ProjectId::parse("Example-Org/Example-Repo").required()
}
