use crate::command::HostEnvironment;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use crate::support::sandbox;

/// Sandbox内のdestinationのdigest。存在しない場合は`None`。
pub(super) fn digest_in_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    destination: &str,
) -> Result<Option<String>> {
    let exists = sandbox::exec(host, sandbox, &["test", "-e", destination])?;
    if !exists.success() {
        return Ok(None);
    }

    let outcome = sandbox::exec(host, sandbox, &["sha256sum", destination])?.require_success()?;
    let text = outcome.stdout_text();
    let digest = text.split_whitespace().next().unwrap_or_default();
    if digest.len() != 64 {
        return Err(Error::new(
            ErrorId::ExternalOutputUnparseable,
            msg!(
                "error-external-output-unparseable",
                program = "sha256sum",
                detail = format!("no digest was reported for {destination}")
            ),
        ));
    }
    Ok(Some(digest.to_string()))
}
