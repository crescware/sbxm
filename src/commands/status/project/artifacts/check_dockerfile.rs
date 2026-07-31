use crate::diagnostics::{Diagnostic, ErrorId};
use crate::hash::sha256_hex;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};

use crate::commands::status::project::{ProjectStatus, Value};

/// 現在のDockerfileと、metadataに記録した適用済み世代の関係。
pub fn check_dockerfile(
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let path = paths.dockerfile();
    match paths::regular_file_exists(&path, PathScope::ProjectPath) {
        Ok(true) => match std::fs::read(&path) {
            Ok(contents) => {
                let digest = sha256_hex(&contents);
                // 変更済みは次の`rebuild`対象であり、破損ではない。
                let value = if digest == metadata.provisioning.dockerfile_sha256 {
                    Value::Ready
                } else {
                    Value::Changed
                };
                status.push("status-item-dockerfile", value);
            }
            Err(error) => {
                status.push("status-item-dockerfile", Value::Mismatch);
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::ProjectPathUnreadable,
                    msg!(
                        "error-project-path-unreadable",
                        path = paths::display(&path),
                        detail = error
                    ),
                ));
            }
        },
        Ok(false) => status.push("status-item-dockerfile", Value::Missing),
        Err(error) => {
            status.push("status-item-dockerfile", Value::Mismatch);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
        }
    }
}
