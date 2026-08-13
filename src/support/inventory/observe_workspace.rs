use std::path::Path;

use crate::project::SandboxName;

use crate::support::sandbox;

use super::{Observed, ProjectState, WorkspaceState};

/// 案件の中立workspace directoryを、runtime stateとは別の事実として観測する。
///
/// 実測するのは、この案件のSandboxのrecordを一覧のなかで特定できた場合だけとする。
/// recordが無い案件、または対応を確定できない案件には、mount元として宣言された
/// directoryが無いため`not-applicable`とする。recordが無いままdirectoryだけが
/// 残っている場合も、sbxmが状態を問う対象は無い。
///
/// 観測できなかったことを不在と同一視せず、どちらの場合も`ready`とは答えない。error
/// にはしない。1案件の観測失敗で一覧全体を失わせないためであり、理由は
/// `status <project-id>`が示す。
pub fn observe_workspace(
    sandbox_name: &SandboxName,
    workspace_root: &Path,
    observed: &Observed,
) -> WorkspaceState {
    match observed {
        Observed::Registered(ProjectState::Running | ProjectState::Stopped) => {
            match sandbox::workspace_exists(workspace_root, sandbox_name) {
                Ok(true) => WorkspaceState::Ready,
                Ok(false) => WorkspaceState::Missing,
                Err(_) => WorkspaceState::NotObserved,
            }
        }
        Observed::Registered(ProjectState::NotCreated)
        | Observed::Missing
        | Observed::Incomplete
        | Observed::Inconsistent => WorkspaceState::NotApplicable,
    }
}
