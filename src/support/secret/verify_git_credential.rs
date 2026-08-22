use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::{GITHUB_HOST, GITHUB_TOKEN_ENV};

/// 既存の `Sandbox` にある `GitHub` credential helper を read-only で確認する。
pub(crate) fn verify_git_credential(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let key = format!("credential.https://{GITHUB_HOST}.helper");
    let outcome = crate::support::sandbox::exec(
        host,
        sandbox,
        &["git", "config", "--global", "--get", &key],
    )?;
    let observed = outcome.stdout_text().trim().to_string();
    match crate::support::sandbox::inner_exit_code(&outcome) {
        Some(0) => {
            if observed.is_empty() || observed == helper() {
                return Ok(());
            }
        }
        // `git config --get` uses status 1 for a missing key.
        Some(1) => return Ok(()),
        _ => return Err(crate::support::sandbox::unobservable(&outcome, &key)),
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::SandboxIdentityMismatch,
            msg!(
                "error-sandbox-identity-mismatch",
                sandbox = sandbox,
                key = key,
                observed = observed,
                expected = helper()
            ),
        )
        .fact(Fact::value(&key))
        .remediation(msg!("remediation-sandbox-identity-mismatch")),
    ))
}

fn helper() -> String {
    format!("!f() {{ echo username=x; echo password=${GITHUB_TOKEN_ENV}; }}; f")
}
