use crate::command::{CommandSpec, HostEnvironment};
use crate::diagnostics::Result;

use super::Prepared;

/// terminalをSSHへ引き渡す。
///
/// `SSHのexit` statusが0なら成功とし、非ゼロは理由を推測せず外部command失敗とする。
pub fn connect(host: &dyn HostEnvironment, prepared: &Prepared) -> Result<()> {
    let spec = CommandSpec::inherit("ssh", &[&prepared.ssh_host]);
    host.run(&spec)?.require_success()?;
    Ok(())
}
