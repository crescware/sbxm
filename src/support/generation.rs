//! Dockerfileの世代。
//!
//! 世代はDockerfileの内容hashで表す。`rebuild`は切替の途中でintentを残すため、工程を
//! 進めるcommandは進める前にintentの不在を確かめる。

use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::hash::sha256_hex;
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, PathScope, ProjectPaths};

/// host側にある現在のDockerfileの世代。
pub fn current_dockerfile_hash(paths: &ProjectPaths) -> Result<String> {
    let path = paths.dockerfile();
    if !paths::regular_file_exists(&path, PathScope::ProjectPath)? {
        return Err(Error::new(
            ErrorId::ProjectPathUnreadable,
            msg!(
                "error-project-path-unreadable",
                path = paths::display(&path),
                detail = "the Dockerfile of this project is absent"
            ),
        ));
    }
    let contents = std::fs::read(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    Ok(sha256_hex(&contents))
}

/// 世代の切替中は工程を進めず、`rebuild`の完了を案内する。
pub fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(msg!(
            "remediation-run-rebuild",
            command = format!("sbxm rebuild {}", metadata.display_id())
        )),
    ))
}
