use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::exec;

/// Sandbox内にpathが存在するか。
pub fn path_exists(host: &dyn HostEnvironment, sandbox: &str, path: &str) -> Result<bool> {
    Ok(exec(host, sandbox, &["test", "-e", path])?.success())
}
