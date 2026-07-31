use std::path::PathBuf;

use crate::diagnostics::Result;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};

use super::ConfigLocation;

/// `~/.sbxm`を`0700`で検証または作成する。
pub fn ensure_config_dir(location: &ConfigLocation) -> Result<PathBuf> {
    let dir = location.dir();
    paths::ensure_private_dir(&dir, PRIVATE_DIR_MODE, PathScope::ConfigDir)?;
    Ok(dir)
}
