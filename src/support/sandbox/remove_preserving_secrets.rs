use crate::boundary::host::{HostEnvironment, remove_sandbox_preserving_secrets};
use crate::diagnostics::Result;
use crate::project::SandboxName;

/// inventoryの削除許可の下で、scoped secretを保持してSandboxだけを削除する。
pub fn remove_preserving_secrets(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    force: bool,
) -> Result<()> {
    remove_sandbox_preserving_secrets(host, name.as_str(), force)
}
