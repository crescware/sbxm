use std::path::{Path, PathBuf};

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use crate::paths::inspect::display;

/// 決定的な一時file名。中断した実行の残骸を次回起動時に検出できるようにする。
pub(super) fn temp_path_for(target: &Path) -> Result<PathBuf> {
    let parent = target.parent().ok_or_else(|| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = "the target has no parent directory"
            ),
        )
    })?;
    let name = target.file_name().ok_or_else(|| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = "the target has no file name"
            ),
        )
    })?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(name);
    temp_name.push(".tmp");
    Ok(parent.join(temp_name))
}
