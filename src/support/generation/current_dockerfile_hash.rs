use crate::diagnostics::{Error, ErrorId, Result};
use crate::hash::sha256_hex;
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
