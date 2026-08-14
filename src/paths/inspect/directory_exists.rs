use std::fs::{self};
use std::path::Path;

use crate::diagnostics::Result;

use crate::paths::scope::PathScope;

use super::{is_symlink, unexpected_type};

/// pathがdirectoryとして存在するかを、symlinkを追跡せずに判定する。
///
/// 不在は`false`で答え、観測できなかった場合は`false`にせず拒否する。symlinkと、
/// directory以外のfile typeは、辿った先を見に行かずに拒否する。
pub fn directory_exists(path: &Path, scope: PathScope) -> Result<bool> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(metadata) => Err(unexpected_type(path, "directory", &metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(scope.unreadable_error(path, &error.to_string())),
    }
}
