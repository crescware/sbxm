use std::path::Path;

use crate::diagnostics::Result;
use crate::paths::scope::PathScope;

use super::create_private_directory::create_private_directory;
use super::ensure_private_dir_with::ensure_private_dir_with;

/// `~/.sbxm`のような、利用者専用directoryを検証または作成する。
///
/// 安全条件は[`ensure_private_dir_with`]に従う。permissionはmkdirの時点で決める。
pub fn ensure_private_dir(path: &Path, mode: u32, scope: PathScope) -> Result<()> {
    ensure_private_dir_with(path, mode, scope, &create_private_directory)
}
