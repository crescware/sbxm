use crate::design::{Inline, VisualState};
use crate::support::inventory::ProjectState;

/// 一覧に並ぶSandboxの状態。
pub fn project_state(state: ProjectState) -> Inline {
    let visual = match state {
        ProjectState::Running => VisualState::Positive,
        ProjectState::Stopped | ProjectState::NotCreated => VisualState::Attention,
    };
    Inline::state(state.as_str(), visual)
}
