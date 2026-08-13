use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::ErrorId;
use crate::metadata::ProjectMetadata;
use crate::paths::{self, PathScope};

use crate::support::daemon;
use crate::support::inventory::{self, ProjectState};
use crate::support::sandbox;

use crate::commands::status::project::{ProjectStatus, Value};

/// Sandboxとworkspaceの状態。
///
/// 対象案件だけを名前の完全一致で突き合わせる。ほかの案件の破損で、この案件の状態が
/// 読めなくなることはない。
///
/// 2つの項目は別々の事実を指す。`status-item-sandbox`はruntimeが持つrecordの状態で
/// あり、その出所は`sbx ls`である。`status-item-workspace`はそのrecordがmount元と
/// して宣言する中立workspace directoryが、host上に在るかである。recordが在ることは
/// directoryが在ることを含まないため、後者はhostを実測して決める。
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
            status.push("status-item-sandbox", Value::NotObserved);
            status.push("status-item-workspace", Value::NotObserved);
            status.global_scope_failure(&error);
            return None;
        }
    };

    match observed {
        Ok(state) => {
            let sandbox = match state {
                ProjectState::Running => Value::Running,
                ProjectState::Stopped => Value::Stopped,
                ProjectState::NotCreated => Value::NotCreated,
            };
            let workspace = match state {
                // Sandboxが無い案件には、状態を問う対象のmount元が無い。
                ProjectState::NotCreated => Value::NotApplicable,
                ProjectState::Running | ProjectState::Stopped => {
                    observe_workspace(metadata, workspace_root, status)
                }
            };
            status.push("status-item-sandbox", sandbox);
            status.push("status-item-workspace", workspace);
            Some(state)
        }
        Err(error) => {
            let value = if error.contains_id(ErrorId::SandboxNameCollision) {
                Value::NotObserved
            } else {
                Value::Mismatch
            };
            status.push("status-item-sandbox", value);
            status.push("status-item-workspace", value);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            None
        }
    }
}

/// 中立workspace directoryが、host上に在るかを実測する。
///
/// 不在と、観測できないことを同じ値にしない。観測できなかった場合はその理由を診断へ
/// 残し、在るともないとも答えない。どちらの場合も`ready`とは答えない。
fn observe_workspace(
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    status: &mut ProjectStatus,
) -> Value {
    let workspace = sandbox::workspace_path(workspace_root, &metadata.sandbox_name());
    match paths::directory_exists(&workspace, PathScope::ProjectPath) {
        Ok(true) => Value::Ready,
        Ok(false) => Value::Missing,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::NotObserved
        }
    }
}
