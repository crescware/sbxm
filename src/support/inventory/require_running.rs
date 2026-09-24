use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;

use crate::support::daemon;

use super::{ProjectState, not_created, state_of};

/// 案件のSandboxが動いていることを確かめる。
///
/// 中を読むだけの操作でも、停止中のSandboxを起動しない。起動は`open`の責務であり、
/// 止まっている案件は`open`へ案内する。
pub fn require_running(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<()> {
    let sandbox = metadata.sandbox_name();
    let entries = daemon::list(host)?;
    match state_of(&entries, metadata, workspace_root)? {
        ProjectState::Running => Ok(()),
        ProjectState::NotCreated => Err(not_created(metadata, sandbox.as_str())),
        ProjectState::Stopped => Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotRunning,
                msg!(
                    "error-sandbox-not-running",
                    sandbox = sandbox.as_str(),
                    observed = ProjectState::Stopped.as_str()
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-sandbox-not-running"))
                    .try_run(format!("sbxm open {}", metadata.display_id())),
            ),
        )),
    }
}
