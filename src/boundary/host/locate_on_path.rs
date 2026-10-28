use std::path::PathBuf;

use super::locate_in_path_value;

/// PATH上でcommandとして起動されるfileを、実行せずに探す。
pub(super) fn locate_on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    locate_in_path_value(program, &path)
}
