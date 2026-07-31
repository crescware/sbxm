use crate::testing::outcome::Checked;

use crate::project::ProjectId;
use crate::testing::project::project_id;

pub fn project() -> Checked<ProjectId> {
    project_id("Example-Org/Example-Repo")
}
