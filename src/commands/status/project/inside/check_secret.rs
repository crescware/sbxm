use crate::command::HostEnvironment;
use crate::diagnostics::ErrorId;
use crate::project::SandboxName;

use crate::support::secret;

use crate::commands::status::project::{ProjectStatus, Value};

pub fn check_secret(host: &dyn HostEnvironment, name: &SandboxName, status: &mut ProjectStatus) {
    // 登録されていることと、そのSandboxが受け取っていることは別である。片方だけを見て
    // 使える状態とは言えない。
    let value = match secret::require_github(host, name.as_str())
        .and_then(|()| secret::require_placeholder_present(host, name.as_str()))
    {
        Ok(()) => Value::Ready,
        Err(error) if error.contains_id(ErrorId::GithubSecretMissing) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Missing
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-secret", value);
}
