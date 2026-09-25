use crate::design::{Inline, VisualState};

use crate::commands::status::project::FileState;

/// 宣言file 1件の、hostまたはSandboxでの状態。
pub fn file_state(state: FileState) -> Inline {
    let visual = match state {
        FileState::Unchanged => VisualState::Positive,
        // どれも壊れてはいないが、次の`apply --files`の結果を変える。
        FileState::Updated
        | FileState::Unplaced
        | FileState::Modified
        | FileState::Unrecorded
        | FileState::Missing
        | FileState::NotObserved
        | FileState::NotObservedStopped => VisualState::Attention,
        // `apply --files`はこの宣言で止まる。
        FileState::Unreadable => VisualState::Negative,
        FileState::NotApplicable => VisualState::Neutral,
    };
    Inline::state(state.as_str(), visual)
}
