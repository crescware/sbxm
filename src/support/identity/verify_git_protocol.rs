use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::{GIT_PROTOCOL, GITHUB_HOST, mismatch};

/// 既存Sandboxの`gh` protocolをread-onlyで確認する。
pub(crate) fn verify_git_protocol(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
    let outcome = sandbox::exec(
        host,
        sandbox,
        &["gh", "config", "get", "git_protocol", "--host", GITHUB_HOST],
    )?;
    if outcome.success() {
        let observed = outcome.stdout_text().trim().to_string();
        if !observed.is_empty() && observed != GIT_PROTOCOL {
            return Err(mismatch(
                sandbox,
                "gh git_protocol",
                &observed,
                GIT_PROTOCOL,
            ));
        }
    }
    Ok(())
}
