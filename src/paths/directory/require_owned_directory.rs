use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::diagnostics::Result;

use crate::paths::inspect::{is_symlink, require_owned_by_current_user, unexpected_type};
use crate::paths::scope::PathScope;

/// 既存directoryが、現在の利用者が所有する通常のdirectoryであることを確認する。
///
/// 保存済みのabsolute pathを信用する前に使う。何も作らず、何も修復しない。
pub fn require_owned_directory(path: &Path, scope: PathScope) -> Result<()> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| scope.unreadable_error(path, &error.to_string()))?;
    if !metadata.is_dir() {
        return Err(unexpected_type(path, "directory", &metadata));
    }
    require_owned_by_current_user(path, metadata.uid(), scope)
}
