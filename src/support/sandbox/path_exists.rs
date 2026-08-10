use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::{exec, inner_exit_code, unobservable};

/// Sandbox内にpathが存在するか。`test -e`の0（存在）と1（不存在）だけを判定結果とし、
/// その他の終了statusは観測不能として返す。
pub fn path_exists(host: &dyn HostEnvironment, sandbox: &str, path: &str) -> Result<bool> {
    let outcome = exec(host, sandbox, &["test", "-e", path])?;
    match inner_exit_code(&outcome) {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(unobservable(&outcome, path)),
    }
}
