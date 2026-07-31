use std::fs::File;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use crate::diagnostics::Result;

use crate::paths::scope::PathScope;

use super::{permission_too_open, require_owned_by_current_user, unexpected_type};

/// 開いたfileが、現在の利用者だけが読み書きできる通常fileであることを確認する。
pub fn require_private_file(file: &File, path: &Path, mode: u32, scope: PathScope) -> Result<()> {
    let metadata = file
        .metadata()
        .map_err(|error| scope.unreadable_error(path, &error.to_string()))?;
    if !metadata.is_file() {
        return Err(unexpected_type(path, "regular file", &metadata));
    }
    require_owned_by_current_user(path, metadata.uid(), scope)?;
    let observed = metadata.permissions().mode();
    if permission_too_open(observed) {
        return Err(scope.permission_error(path, observed, mode));
    }
    Ok(())
}
