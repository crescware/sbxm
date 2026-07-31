use crate::testing::outcome::Checked;

use crate::project::SandboxName;

use super::canonical;

pub fn sandbox() -> Checked<SandboxName> {
    Ok(SandboxName::derive(&canonical()?))
}
