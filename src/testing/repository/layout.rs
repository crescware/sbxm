use crate::testing::outcome::Checked;

use crate::project::SandboxLayout;

use super::canonical;

pub fn layout() -> Checked<SandboxLayout> {
    Ok(SandboxLayout::new(&canonical()?))
}
