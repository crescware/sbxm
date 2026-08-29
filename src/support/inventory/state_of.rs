use std::path::Path;

use crate::boundary::host::protocol::{SandboxEntry, SandboxState};
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;

use crate::support::sandbox;

use super::{ProjectState, single};

/// 1案件の現在の状態を、取得済みの一覧から決める。
///
/// 一覧全体が成立している必要はない。対象と無関係な案件の破損によって、この案件の
/// 状態が読めなくなることを避ける。対応が矛盾する場合は、正常な状態として返さない。
pub fn state_of(
    entries: &[SandboxEntry],
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<ProjectState> {
    let name = metadata.sandbox_name();
    let Some(entry) = single(entries, name.as_str())? else {
        return Ok(ProjectState::NotCreated);
    };
    sandbox::verify_identity(entry, &name, workspace_root)?;
    Ok(match entry.state {
        SandboxState::Running => ProjectState::Running,
        SandboxState::Stopped => ProjectState::Stopped,
    })
}
