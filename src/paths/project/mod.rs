//! 案件のhost path。

mod project_parent;
mod project_paths;
mod working_directory_unusable;

pub use project_parent::ProjectParent;
pub use project_paths::ProjectPaths;
use working_directory_unusable::working_directory_unusable;

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;
