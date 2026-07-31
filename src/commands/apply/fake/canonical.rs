use crate::testing::outcome::Checked;

use crate::project::CanonicalProjectId;

use super::project;

pub fn canonical() -> Checked<CanonicalProjectId> {
    Ok(project()?.canonical())
}
