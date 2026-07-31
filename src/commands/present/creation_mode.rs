use crate::design::{Inline, VisualState};
use crate::metadata::CreationMode;

/// worktreeの作成mode。良し悪しではなく構成の別である。
pub fn creation_mode(mode: CreationMode) -> Inline {
    Inline::state(mode.to_string(), VisualState::Neutral)
}
