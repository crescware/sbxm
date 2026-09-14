use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

/// hostのSSH Agentへ、Sandboxの中から到達できたという診断。
///
/// 転送はDocker Sandboxesの設定で止める。Sandboxを消しても、作り直したSandboxは
/// 同じ設定のdaemonから再びagentを受け取るうえ、scoped secretまで失う。
pub fn ssh_agent_exposed(sandbox: &str, observed: &[&str]) -> Diagnostic {
    Diagnostic::new(
        ErrorId::SshAgentExposed,
        msg!(
            "security-ssh-agent-exposed-description",
            sandbox = sandbox,
            observed = observed.join(", ")
        ),
    )
    .remediation(
        Remediation::text(msg!("security-ssh-agent-exposed-remediation"))
            .try_run("sbx settings set ssh.agentForwardingEnabled false")
            .try_run("sbx daemon restart"),
    )
}
