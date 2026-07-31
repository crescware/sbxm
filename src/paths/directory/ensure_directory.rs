use std::fs;
use std::path::Path;

use crate::diagnostics::{Error, ErrorId, Result, fail};
use crate::msg;

use crate::paths::inspect::{display, is_symlink, unexpected_type};
use crate::paths::scope::PathScope;

/// 案件が使うdirectoryを検証または作成する。
///
/// 新規directoryのpermissionは利用者のumaskに従う。symlinkと既存の非directoryは
/// 内容を変更せず拒否する。
pub fn ensure_directory(path: &Path) -> Result<()> {
    if is_symlink(path) {
        return Err(PathScope::ProjectPath.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(metadata) => Err(unexpected_type(path, "directory", &metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(path)
            .map_err(|error| {
                Error::new(
                    ErrorId::AtomicWriteFailed,
                    msg!(
                        "error-atomic-write-failed",
                        path = display(path),
                        detail = error
                    ),
                )
            }),
        Err(error) => fail(
            ErrorId::ProjectPathUnreadable,
            msg!(
                "error-project-path-unreadable",
                path = display(path),
                detail = error
            ),
        ),
    }
}
