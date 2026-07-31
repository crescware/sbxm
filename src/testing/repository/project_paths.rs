use crate::testing::outcome::{Checked, Required};

use crate::paths::{ProjectParent, ProjectPaths};

use super::canonical;

pub fn project_paths(dir: &std::path::Path) -> Checked<ProjectPaths> {
    let parent = ProjectParent::at(dir).required_because("valid parent directory")?;
    let paths = ProjectPaths::derive(&parent, &canonical()?);
    std::fs::create_dir_all(paths.sbxm_dir()).required_because("create .sbxm")?;
    Ok(paths)
}
