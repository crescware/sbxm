use std::fs::{self};
use std::path::{Path, PathBuf};

use super::lexically_standardize;

/// symlinkを解決できない場合は宣言されたpathのまま比較する。
pub fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| lexically_standardize(path))
}
