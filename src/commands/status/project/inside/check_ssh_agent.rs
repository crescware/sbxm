use crate::boundary::host::HostEnvironment;
use crate::project::SandboxName;

use crate::support::sandbox;

use crate::commands::status::project::{ProjectStatus, Value};

/// SSH Agentが露出していないこと。
///
/// 露出していないことは、検査commandが答えた場合にだけ言える。検査自体が成立しない
/// 場合を`not-exposed`へ丸めない。
pub fn check_ssh_agent(host: &dyn HostEnvironment, name: &SandboxName, status: &mut ProjectStatus) {
    let value = match sandbox::ssh_agent_is_exposed(host, name.as_str()) {
        Ok(observed) if !observed.is_empty() => {
            status
                .diagnostics
                .push(sandbox::ssh_agent_exposed(name.as_str(), &observed));
            Value::Exposed
        }
        Ok(_) => Value::NotExposed,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::NotObserved
        }
    };
    status.push("status-item-ssh-agent", value);
}
