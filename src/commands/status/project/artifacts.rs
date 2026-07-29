//! 案件directoryと、世代ごとの成果物の診断。

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, ErrorId};
use crate::hash::{sha256_hex, short_hex};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::project::SandboxName;

use crate::support::image::{self, LABEL_CANONICAL_ID, LABEL_DOCKERFILE_SHA256};

use super::{ProjectStatus, Value};

/// project rootとhost cloneの有無。
pub(super) fn check_directory(paths: &ProjectPaths, status: &mut ProjectStatus) {
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

/// 現在のDockerfileと、metadataに記録した適用済み世代の関係。
pub(super) fn check_dockerfile(
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

/// 適用済み世代のimageが、この案件のものとして存在するか。
pub(super) fn check_image(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let generation = &metadata.provisioning.dockerfile_sha256;
    let image = image::image_name(name, generation);

    let value = match image::inspect(host, &image) {
        Ok(Some(identity)) => {
            let declares_project =
                identity.labels.get(LABEL_CANONICAL_ID) == Some(&metadata.canonical_id.to_string());
            let declares_generation =
                identity.labels.get(LABEL_DOCKERFILE_SHA256) == Some(generation);
            if declares_project && declares_generation {
                Value::Ready
            } else {
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::ImageUnusable,
                    msg!(
                        "error-image-unusable",
                        image = image,
                        detail = "the labels do not declare this project and generation"
                    ),
                ));
                Value::Mismatch
            }
        }
        Ok(None) => Value::Missing,
        Err(error) => {
            status.global_scope_failure(&error);
            Value::Mismatch
        }
    };
    status.push("status-item-image", value);
}

/// 適用済み世代のTemplate archive。
pub(super) fn check_archive(
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let archive = paths.template_archive(short_hex(&metadata.provisioning.dockerfile_sha256));
    let value = match paths::regular_file_exists(&archive, PathScope::ProjectPath) {
        Ok(true) => Value::Ready,
        Ok(false) => Value::Missing,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-template-archive", value);
}

#[cfg(test)]
#[path = "artifacts_test.rs"]
mod artifacts_test;
