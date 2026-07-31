use std::fs::{self};
use std::path::Path;

/// pathがsymlinkかどうか。存在しない場合は`false`。
pub fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}
