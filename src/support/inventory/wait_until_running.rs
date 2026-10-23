use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SandboxEntry;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;

use crate::design::Remediation;
use crate::support::daemon;

use super::{Poll, ProjectState, state_of};

/// runningになるまで待ち、runningと読めた一覧を返す。状態は毎回structured outputから読む。
///
/// 起動の前に読んだ一覧は、起動の後にはもう正しくない。呼び出し元は、返った一覧を
/// 起動の後の観測に使い、同じ一覧を読み直さない。
pub fn wait_until_running(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    poll: Poll,
) -> Result<Vec<SandboxEntry>> {
    let name = metadata.sandbox_name();
    let deadline = poll.deadline();
    loop {
        let entries = daemon::list(host)?;
        let observed = state_of(&entries, metadata, workspace_root)?;
        if observed == ProjectState::Running {
            return Ok(entries);
        }
        if poll.expired(deadline) {
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
        poll.pause();
    }
}
