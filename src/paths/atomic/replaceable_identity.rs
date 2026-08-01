use std::fs::{self};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use crate::paths::inspect::{
    FileIdentity, display, is_symlink, permission_too_open, unexpected_type,
};
use crate::paths::scope::PathScope;

/// 置き換え対象として妥当なfileのidentity。
pub(super) fn replaceable_identity(target: &Path, mode: u32) -> Result<FileIdentity> {
    if is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(target).map_err(|error| {
        Error::single(
            Diagnostic::new(
                ErrorId::AtomicWriteFailed,
                msg!("error-atomic-write-failed"),
            )
            .fact(Fact::path(&display(target)))
            .fact(Fact::cause(&error.to_string())),
        )
    })?;
    if !metadata.is_file() {
        return Err(unexpected_type(target, "regular file", &metadata));
    }
    let observed = metadata.permissions().mode();
    if permission_too_open(observed) {
        return Err(PathScope::ProjectPath.permission_error(target, observed, mode));
    }
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
