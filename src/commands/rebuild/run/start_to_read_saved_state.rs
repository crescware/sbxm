use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;

use crate::design::ProgressSink;
use crate::support::inventory::{self, Poll};

/// 保存されていない作業を読むために、停止しているSandboxを起動する。
///
/// `rebuild`はこのSandboxをこれから作り直す。状態を読むためだけの起動を利用者へ
/// 求めない。起動できる状態かどうかは`start`が起動前に確かめる。
pub(super) fn start_to_read_saved_state(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    stopped: bool,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    if !stopped {
        return Ok(());
    }
    inventory::start(host, metadata, workspace_root, progress)?;
    inventory::wait_until_running(host, metadata, workspace_root, poll)?;
    Ok(())
}
