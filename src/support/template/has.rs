use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::find;

/// 指定したTemplate名がruntimeから観測できるか。
pub fn has(host: &dyn HostEnvironment, name: &str) -> Result<bool> {
    Ok(find(host, name)?.is_some())
}
