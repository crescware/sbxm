use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::exec;

/// Sandbox内のcommandの標準出力。
pub fn read(host: &dyn HostEnvironment, sandbox: &str, args: &[&str]) -> Result<String> {
    let outcome = exec(host, sandbox, args)?.require_success()?;
    Ok(outcome.stdout_text().trim().to_string())
}
