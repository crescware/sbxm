use crate::boundary::host::protocol::SandboxState;
use crate::design::{Inline, VisualState};

/// 構築後のSandboxの状態。
pub fn sandbox_state(state: SandboxState) -> Inline {
    let visual = match state {
        SandboxState::Running => VisualState::Positive,
        SandboxState::Stopped => VisualState::Attention,
    };
    Inline::state(state.as_str(), visual)
}
