use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::support::repository;

use crate::commands::status::project::{ProjectStatus, Value};

/// project rootとhost cloneの有無。
///
/// hostにあるrepositoryを登録した案件は、登録したそのrepositoryを見る。利用者の
/// repositoryであり、sbxmが取ったcloneではないため、host repositoryとして示す。
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
    let item = if metadata.repository.host_path().is_some() {
        "status-item-host-repository"
    } else {
        "status-item-host-clone"
    };
    status.push(
        item,
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
