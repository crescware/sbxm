//! directoryの検証と作成。
//!
//! symlinkと既存の非directoryは、内容を変更せず拒否する。permissionが過剰な既存
//! directoryも修復しない。

use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::Path;

use crate::error::{Error, ErrorId, Result, fail};
use crate::msg;

use super::inspect::{
    display, is_symlink, permission_too_open, require_owned_by_current_user, unexpected_type,
};
use super::scope::PathScope;

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

#[cfg(test)]
#[path = "directory_test.rs"]
mod directory_test;
