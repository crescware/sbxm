use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use crate::paths::inspect::{
    display, is_symlink, permission_too_open, require_owned_by_current_user, unexpected_type,
};
use crate::paths::scope::PathScope;

/// `~/.sbxm`のような、利用者専用directoryを検証または作成する。
///
/// symlinkは拒否し、既存directoryの所有者が別accountの場合、またはpermissionが
/// 過剰な場合は、作成も修復もしない。
pub fn ensure_private_dir(path: &Path, mode: u32, scope: PathScope) -> Result<()> {
    if is_symlink(path) {
        return Err(scope.symlink_error(path));
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(unexpected_type(path, "directory", &metadata));
            }
            // 別accountが先に作った`0700`のdirectoryを、自分のものとして使わない。
            require_owned_by_current_user(path, metadata.uid(), scope)?;
            let observed = metadata.permissions().mode();
            if permission_too_open(observed) {
                return Err(scope.permission_error(path, observed, mode));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // permissionはmkdirの時点で決める。作ってから絞ると、そのあいだに別のprocess
            // が広いmodeのdirectoryを観測し、自分のものではないとして拒否してしまう。
            let failed = |error: std::io::Error| {
                Error::new(
                    ErrorId::AtomicWriteFailed,
                    msg!(
                        "error-atomic-write-failed",
                        path = display(path),
                        detail = error
                    ),
                )
            };
            fs::DirBuilder::new()
                .recursive(true)
                .mode(mode)
                .create(path)
                .map_err(failed)?;
            // umaskはbitを落とすだけで足さない。要求したmodeを確定させる。
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(failed)?;
            Ok(())
        }
        Err(error) => Err(scope.unreadable_error(path, &error.to_string())),
    }
}
