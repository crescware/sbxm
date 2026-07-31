use crate::paths::ProjectPaths;

use crate::commands::status::project::{ProjectStatus, Value};

/// project rootとhost cloneの有無。
pub fn check_directory(paths: &ProjectPaths, status: &mut ProjectStatus) {
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
        if paths.host_clone().join(".git").exists() {
            Value::Ready
        } else {
            Value::Missing
        },
    );
}
