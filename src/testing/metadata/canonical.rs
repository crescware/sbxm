use crate::testing::outcome::{Checked, Required};

use crate::project::{CanonicalProjectId, ProjectId};

pub fn canonical(value: &str) -> Checked<CanonicalProjectId> {
    Ok(ProjectId::parse(value)
        .required_because("valid project id")?
        .canonical())
}
