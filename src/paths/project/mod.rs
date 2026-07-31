//! 案件のhost path。

mod project_parent;
mod project_paths;

pub use project_parent::ProjectParent;
pub use project_paths::ProjectPaths;

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;
