use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::support::repository;

use crate::commands::status::project::{ProjectStatus, Value};

/// project rootとhost cloneの有無。
///
/// hostにあるrepositoryを登録した案件は、登録したそのrepositoryを見る。
pub fn check_directory(
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    status.push(
        "status-item-project-root",
        if paths.root().is_dir() {
            Value::Ready
        } else {
            Value::Missing
        },
    );
    status.push(
        "status-item-host-clone",
        if repository::host_repository(paths, metadata)
            .join(".git")
            .exists()
        {
            Value::Ready
        } else {
            Value::Missing
        },
    );
}
