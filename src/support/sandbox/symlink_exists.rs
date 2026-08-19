use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::{exec, inner_exit_code, unobservable};

/// Sandbox内のpathがsymlinkかを、0/1以外の終了値を不在と読まずに確認する。
pub fn symlink_exists(host: &dyn HostEnvironment, sandbox: &str, path: &str) -> Result<bool> {
    let outcome = exec(host, sandbox, &["test", "-h", path])?;
    match inner_exit_code(&outcome) {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(unobservable(&outcome, path)),
    }
}
