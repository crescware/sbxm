use crate::testing::outcome::{Checked, Required};

use crate::project::{CanonicalProjectId, ProjectId};

pub fn canonical() -> Checked<CanonicalProjectId> {
    Ok(ProjectId::parse("example-org/example-repo")
        .required()?
        .canonical())
}
