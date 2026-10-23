use crate::diagnostics::Result;

use super::{Inside, inner_exit_code, unobservable};

/// Sandbox内にpathが存在するか。`test -e`の0（存在）と1（不存在）だけを判定結果とし、
/// その他の終了statusは観測不能として返す。
pub fn path_exists(inside: &dyn Inside, path: &str) -> Result<bool> {
    let outcome = inside.exec(&["test", "-e", path])?;
    match inner_exit_code(&outcome) {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(unobservable(&outcome, path)),
    }
}
