use std::path::Path;
use std::time::Instant;

use crate::command::HostEnvironment;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;

use crate::design::Remediation;
use crate::support::daemon;

use super::{Poll, ProjectState, state_of};

/// runningになるまで待つ。状態は毎回structured outputから読む。
pub fn wait_until_running(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    poll: Poll,
) -> Result<()> {
    let name = metadata.sandbox_name();
    let deadline = Instant::now() + poll.limit;
    loop {
        let entries = daemon::list(host)?;
        let observed = state_of(&entries, metadata, workspace_root)?;
        if observed == ProjectState::Running {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::SandboxNotRunning,
                    msg!(
                        "error-sandbox-not-running",
                        sandbox = name,
                        observed = observed
                    ),
                )
                .remediation(
                    Remediation::text(msg!("remediation-diagnose-project"))
                        .try_run(format!("sbxm status {}", metadata.display_id())),
                ),
            ));
        }
        std::thread::sleep(poll.interval);
    }
}
