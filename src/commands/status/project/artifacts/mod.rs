//! 案件directoryと、世代ごとの成果物の診断。

mod check_directory;
mod check_dockerfile;
mod check_image;

pub(super) use check_directory::check_directory;
pub(super) use check_dockerfile::check_dockerfile;
pub(super) use check_image::check_image;

#[cfg(test)]
#[path = "artifacts_test.rs"]
mod artifacts_test;
