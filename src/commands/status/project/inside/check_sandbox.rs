use std::path::Path;

use crate::command::HostEnvironment;
use crate::metadata::ProjectMetadata;

use crate::support::daemon;
use crate::support::inventory::{self, ProjectState};

use crate::commands::status::project::{ProjectStatus, Value};

/// Sandboxとworkspaceの状態。
///
/// 対象案件だけを名前の完全一致で突き合わせる。ほかの案件の破損で、この案件の状態が
/// 読めなくなることはない。
pub fn check_sandbox(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    status: &mut ProjectStatus,
) -> Option<ProjectState> {
    let observed = match daemon::list(host) {
        Ok(entries) => inventory::state_of(&entries, metadata, workspace_root),
        Err(error) => {
            // 一覧そのものを読めないのはglobal環境の問題である。
            status.push("status-item-sandbox", Value::Mismatch);
            status.push("status-item-workspace", Value::Mismatch);
            status.global_scope_failure(&error);
            return None;
        }
    };

    match observed {
        Ok(state) => {
            let (sandbox, workspace) = match state {
                ProjectState::Running => (Value::Running, Value::Ready),
                ProjectState::Stopped => (Value::Stopped, Value::Ready),
                ProjectState::NotCreated => (Value::NotCreated, Value::NotApplicable),
            };
            status.push("status-item-sandbox", sandbox);
            status.push("status-item-workspace", workspace);
            Some(state)
        }
        Err(error) => {
            status.push("status-item-sandbox", Value::Mismatch);
            status.push("status-item-workspace", Value::Mismatch);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            None
        }
    }
}
