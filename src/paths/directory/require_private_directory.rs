use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use crate::diagnostics::Result;

use crate::paths::inspect::{
    is_symlink, permission_too_open, require_owned_by_current_user, unexpected_type,
};
use crate::paths::scope::PathScope;

/// 既存directoryが、現在の利用者だけが読み書きできるdirectoryであることを確認する。
///
/// 保存済みのabsolute pathを信用する前に使う。何も作らず、何も修復しない。
pub fn require_private_directory(path: &Path, mode: u32, scope: PathScope) -> Result<()> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return Err(scope.unreadable_error(path, &error.to_string())),
    };
    if !metadata.is_dir() {
        return Err(unexpected_type(path, "directory", &metadata));
    }
    require_owned_by_current_user(path, metadata.uid(), scope)?;
    let observed = metadata.permissions().mode();
    if permission_too_open(observed) {
        return Err(scope.permission_error(path, observed, mode));
    }
    Ok(())
}
