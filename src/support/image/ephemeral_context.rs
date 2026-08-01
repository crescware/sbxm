use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};

use super::BUILD_CONTEXT_PREFIX;

/// `docker build`へ渡す、空で私有な一時directory。
pub(super) fn ephemeral_context() -> Result<tempfile::TempDir> {
    let context = tempfile::Builder::new()
        .prefix(BUILD_CONTEXT_PREFIX)
        .permissions(std::fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .tempdir()
        .map_err(|error| {
            Error::single(
                Diagnostic::new(
                    ErrorId::AtomicWriteFailed,
                    msg!("error-atomic-write-failed"),
                )
                .fact(Fact::path(BUILD_CONTEXT_PREFIX))
                .fact(Fact::cause(&error.to_string())),
            )
        })?;

    let path = context.path().to_path_buf();
    if paths::is_symlink(&path) {
        return Err(PathScope::ProjectPath.symlink_error(&path));
    }
    let resolved = fs::canonicalize(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    let entries = fs::read_dir(&resolved)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&resolved, &error.to_string()))?
        .count();
    if entries != 0 {
        return Err(Error::new(
            ErrorId::BuildContextNotEmpty,
            msg!(
                "error-build-context-not-empty",
                path = paths::display(&resolved),
                observed = entries
            ),
        ));
    }
    Ok(context)
}
