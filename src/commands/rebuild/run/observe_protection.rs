use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::project::{SandboxLayout, SandboxName};

use crate::design::ProgressSink;
use crate::support::inventory::{Poll, ProjectState};
use crate::support::protection::{self, DestructiveOperation, ProtectionSnapshot, Request};

use super::start_to_read_saved_state;

/// 現在のSandbox状態を観測し、保護snapshotを作る。
///
/// Sandboxが無ければ何も観測せず空のsnapshotを返す。停止していれば、保存されていない
/// 作業を読むために先に起動する。`prepare`と`execute`の両方が、それぞれの時点の最新
/// stateからこの関数を呼ぶ。
pub(super) fn observe_protection(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    name: &SandboxName,
    state: ProjectState,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<ProtectionSnapshot> {
    if state == ProjectState::NotCreated {
        return Ok(protection::gate::assess_absent(
            DestructiveOperation::Rebuild,
            metadata.display_id(),
            name,
        ));
    }
    start_to_read_saved_state(
        host,
        metadata,
        name,
        state == ProjectState::Stopped,
        workspace_root,
        poll,
        progress,
    )?;
    let layout = SandboxLayout::new(metadata.canonical_id());
    let request = Request::new(DestructiveOperation::Rebuild, name, &layout, metadata);
    protection::gate::assess(host, &request)
}
