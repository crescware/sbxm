use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use crate::paths::inspect::display;
use crate::paths::scope::PathScope;

use super::require_private_directory;

/// `~/.sbxm`のような、利用者専用directoryを検証または作成する。
///
/// symlinkは拒否し、既存directoryの所有者が別accountの場合、またはpermissionが
/// 過剰な場合は、作成も修復もしない。
pub fn ensure_private_dir(path: &Path, mode: u32, scope: PathScope) -> Result<()> {
    match fs::symlink_metadata(path) {
        // 別accountが先に作った`0700`のdirectoryを、自分のものとして使わない。
        Ok(_) => require_private_directory(path, mode, scope),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // permissionはmkdirの時点で決める。作ってから絞ると、そのあいだに別のprocess
            // が広いmodeのdirectoryを観測し、自分のものではないとして拒否してしまう。
            let failed = |error: std::io::Error| {
                Error::single(
                    Diagnostic::new(
                        ErrorId::AtomicWriteFailed,
                        msg!("error-atomic-write-failed"),
                    )
                    .fact(Fact::path(&display(path)))
                    .fact(Fact::cause(&error.to_string())),
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
