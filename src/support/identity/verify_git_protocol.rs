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
    match sandbox::inner_exit_code(&outcome) {
        Some(0) => {
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
        // `gh config get` uses status 1 when the host has no explicit setting.
        Some(1) => {}
        _ => return Err(sandbox::unobservable(&outcome, "gh git_protocol")),
    }
    Ok(())
}
