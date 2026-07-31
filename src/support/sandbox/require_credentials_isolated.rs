use crate::command::HostEnvironment;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use crate::design::Remediation;

use super::ssh_agent_is_exposed;

/// 作成または再作成したSandboxが、hostのcredentialから隔離されていること。
pub fn require_credentials_isolated(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let observed = ssh_agent_is_exposed(host, sandbox)?;
    if observed.is_empty() {
        return Ok(());
    }
    Err(Error::single(
        crate::diagnostics::Diagnostic::new(
            ErrorId::SshAgentExposed,
            msg!(
                "security-ssh-agent-exposed-description",
                sandbox = sandbox,
                observed = observed.join(", ")
            ),
        )
        .remediation(
            Remediation::text(msg!("security-ssh-agent-exposed-remediation"))
                .try_run(format!("sbx rm {sandbox}")),
        ),
    ))
}
