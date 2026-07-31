use crate::command::HostEnvironment;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;
use crate::project::SandboxName;

use crate::support::sandbox;

use crate::commands::status::project::{ProjectStatus, Value};
use crate::design::Remediation;

/// SSH Agentが露出していないこと。
///
/// 露出していないことは、検査commandが答えた場合にだけ言える。検査自体が成立しない
/// 場合を`not-exposed`へ丸めない。
pub fn check_ssh_agent(host: &dyn HostEnvironment, name: &SandboxName, status: &mut ProjectStatus) {
    let value = match sandbox::ssh_agent_is_exposed(host, name.as_str()) {
        Ok(observed) if !observed.is_empty() => {
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::SshAgentExposed,
                    msg!(
                        "security-ssh-agent-exposed-description",
                        sandbox = name,
                        observed = observed.join(", ")
                    ),
                )
                .remediation(
                    Remediation::text(msg!("security-ssh-agent-exposed-remediation"))
                        .try_run(format!("sbx rm {name}")),
                ),
            );
            Value::Exposed
        }
        Ok(_) => Value::NotExposed,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-ssh-agent", value);
}
