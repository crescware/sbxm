use crate::boundary::host::HostEnvironment;
use crate::diagnostics::ErrorId;
use crate::project::SandboxName;

use crate::support::secret;

use crate::commands::status::project::{ProjectStatus, Value};

pub fn check_secret(host: &dyn HostEnvironment, name: &SandboxName, status: &mut ProjectStatus) {
    // 見るのは登録があることだけとする。Sandboxの中のgitがそれを提示できるかどうかは
    // credential helperの観測が持ち、足りなければ準備の続きとして埋まる。
    let value = match secret::require_github(host, name.as_str()) {
        Ok(_) => Value::Ready,
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
            Value::NotObserved
        }
    };
    status.push("status-item-secret", value);
}
