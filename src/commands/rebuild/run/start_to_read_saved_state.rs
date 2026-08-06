use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::project::SandboxName;

use crate::design::ProgressSink;
use crate::support::inventory::{self, Poll};

/// 保存されていない作業を読むために、停止しているSandboxを起動する。
///
/// `rebuild`はこのSandboxをこれから作り直す。状態を読むためだけの起動を利用者へ
/// 求めない。
pub(super) fn start_to_read_saved_state(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    name: &SandboxName,
    stopped: bool,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    if !stopped {
        return Ok(());
    }
    inventory::start(host, name.as_str(), progress)?;
    inventory::wait_until_running(host, metadata, workspace_root, poll)?;
    Ok(())
}
