use crate::boundary::host::HostEnvironment;
use crate::metadata::ProjectMetadata;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::inventory::ProjectState;

use crate::commands::status::project::repository::{check_bare_repository, check_worktrees};
use crate::commands::status::project::{ProjectStatus, Value};

use super::{check_secret, check_ssh_agent};

/// Sandbox内部の検査。
///
/// Sandboxがない場合は`not-applicable`、停止中は状態を変えないため検査せず
/// `not-observed-stopped`とする。状態そのものを観測できなかった場合は、Sandboxが
/// 無いことにせず`not-observed`とする。
pub fn check_inside(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    state: Option<ProjectState>,
    status: &mut ProjectStatus,
) {
    let inner = [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ];
    let uniform = match state {
        Some(ProjectState::NotCreated) => Some(Value::NotApplicable),
        // read-onlyの検査でもSandboxを起動し得るため実行しない。
        Some(ProjectState::Stopped) => Some(Value::NotObservedStopped),
        None => Some(Value::NotObserved),
        Some(ProjectState::Running) => None,
    };
    if let Some(value) = uniform {
        for item in inner {
            status.push(item, value);
        }
        return;
    }

    check_secret(host, name, status);
    let layout = SandboxLayout::new(metadata.canonical_id());
    check_bare_repository(host, name, &layout, status);
    check_worktrees(host, name, &layout, metadata, status);
    check_ssh_agent(host, name, status);
}
