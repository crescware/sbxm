use crate::diagnostics::{Error, Result};

use super::{Inside, ssh_agent_exposed, ssh_agent_is_exposed};

/// 作成または再作成したSandboxが、hostのcredentialから隔離されていること。
///
/// Docker Sandboxesは、既定でhostのSSH Agentを各Sandboxへ転送する（v0.42.0から
/// `ssh.agentForwardingEnabled`で無効化できる）。sbxmは自身が起動する`sbx`から
/// `SSH_AUTH_SOCK`を外すが、転送の可否はdaemon側の設定で決まるため、隔離は
/// 作成したSandboxの中から観測して確かめる。
pub fn require_credentials_isolated(inside: &dyn Inside, sandbox: &str) -> Result<()> {
    let observed = ssh_agent_is_exposed(inside)?;
    if observed.is_empty() {
        return Ok(());
    }
    Err(Error::single(ssh_agent_exposed(sandbox, &observed)))
}
