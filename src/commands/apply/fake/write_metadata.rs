use crate::testing::outcome::Checked;

use crate::config::ConfigLocation;
use crate::metadata::RebuildIntent;
use crate::paths::{ProjectParent, ProjectPaths};

use super::register_project;

pub fn write_metadata(
    location: &ConfigLocation,
    parent: &ProjectParent,
    rebuild: Option<RebuildIntent>,
) -> Checked<ProjectPaths> {
    register_project(location, parent, "Example-Org/Example-Repo", rebuild)
}
