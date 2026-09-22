use crate::boundary::host::protocol::SandboxEntry;
use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

/// 現在のSandbox一覧。
pub fn list(host: &dyn HostEnvironment) -> Result<Vec<SandboxEntry>> {
    super::list_with_timeout(host, TimeoutClass::SandboxLifecycle)
}
