use crate::hash::short_hex;
use crate::metadata::ProjectMetadata;
use crate::paths::{self, PathScope, ProjectPaths};

use crate::commands::status::project::{ProjectStatus, Value};

/// 適用済み世代のTemplate archive。
pub fn check_archive(paths: &ProjectPaths, metadata: &ProjectMetadata, status: &mut ProjectStatus) {
    let archive = paths.template_archive(short_hex(&metadata.provisioning.dockerfile_sha256));
    let value = match paths::regular_file_exists(&archive, PathScope::ProjectPath) {
        Ok(true) => Value::Ready,
        Ok(false) => Value::Missing,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::NotObserved
        }
    };
    status.push("status-item-template-archive", value);
}
