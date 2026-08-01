use std::fs::{self, File};
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use crate::paths::inspect::{display, is_symlink, unexpected_type};
use crate::paths::scope::PathScope;

/// 検証済みの一時fileを、同じdirectory内の正式pathへatomicに移す。
///
/// 内容をこのprocessが組み立てられない成果物、たとえば外部commandが書いたarchiveを、
/// 検証を終えてから置き換えるために使う。
pub fn atomic_rename_into_place(temp: &Path, target: &Path) -> Result<()> {
    if is_symlink(temp) {
        return Err(PathScope::ProjectPath.symlink_error(temp));
    }
    if is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(temp)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(temp, &error.to_string()))?;
    if !metadata.is_file() {
        return Err(unexpected_type(temp, "regular file", &metadata));
    }

    fs::rename(temp, target).map_err(|error| {
        Error::single(
            Diagnostic::new(
                ErrorId::AtomicWriteFailed,
                msg!("error-atomic-write-failed"),
            )
            .fact(Fact::path(&display(target)))
            .fact(Fact::cause(&error.to_string())),
        )
    })?;
    if let Some(parent) = target.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}
