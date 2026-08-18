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
    if !outcome.success() {
        return Ok(());
    }
    let observed = outcome.stdout_text().trim().to_string();
    if observed.is_empty() || observed == helper() {
        return Ok(());
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
