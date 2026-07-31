use std::path::Path;

use crate::diagnostics::Result;
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope, atomic_create};

/// 編集結果をconfigへ書く。既存fileは置き換え、無ければ作る。
pub(super) fn write_config(path: &Path, updated: &str) -> Result<()> {
    if paths::regular_file_exists(path, PathScope::ConfigFile)? {
        crate::paths::atomic_replace(path, updated, PRIVATE_FILE_MODE)?;
    } else {
        atomic_create(path, updated, PRIVATE_FILE_MODE)?;
    }
    Ok(())
}
