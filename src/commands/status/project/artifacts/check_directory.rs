use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;

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
    // hostにあるrepositoryは、登録したgit directoryそのものを見る。host cloneは、
    // cloneが済んだことを`.git`の有無でだけ言える。
    let (item, present) = match metadata.repository.host_path() {
        Some(repository) => ("status-item-host-repository", repository.is_dir()),
        None => (
            "status-item-host-clone",
            paths.host_clone().join(".git").exists(),
        ),
    };
    status.push(
        item,
        if present {
            Value::Ready
        } else {
            Value::Missing
        },
    );
}
