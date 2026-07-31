use std::fs::{self};
use std::path::Path;

use crate::diagnostics::Result;

use crate::paths::scope::PathScope;

use super::{is_symlink, unexpected_type};

/// pathが通常fileとして存在するかを、symlinkを追跡せずに判定する。
///
/// symlink、directory、特殊fileは、内容を変更せず拒否する。
pub fn regular_file_exists(path: &Path, scope: PathScope) -> Result<bool> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(metadata) => Err(unexpected_type(path, "regular file", &metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(scope.unreadable_error(path, &error.to_string())),
    }
}
