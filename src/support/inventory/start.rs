use std::path::Path;

use crate::command::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::msg;

use crate::design::ProgressSink;

use crate::support::sandbox;

use super::require_workspace;

/// 非対話でSandboxを起動する。
///
/// 起動には、recordがmount元として宣言するworkspace directoryがhostに在ることが要る。
/// runtime stateはその実在を含まないため、起動を試す前に実測し、欠けている場合は
/// 起動を求めずに拒否する。
pub fn start(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let name = metadata.sandbox_name();
    require_workspace(metadata, &name, workspace_root)?;

    progress.step(msg!("progress-starting-sandbox"));
    let command = sandbox::relayed(&["exec", name.as_str(), "--", "/bin/true"])
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run_with_terminal(&command, progress)?
        .require_success()?;
    Ok(())
}
