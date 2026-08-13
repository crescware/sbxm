use crate::design::{Inline, VisualState};
use crate::support::inventory::WorkspaceState;

/// 一覧に並ぶ中立workspace directoryの実在。
///
/// 不在は起動条件の欠落であり、案件の成果物が失われた状態ではない。registryとの
/// 食い違いと同じ失敗としては示さず、注意として示す。
pub fn workspace_state(state: WorkspaceState) -> Inline {
    let visual = match state {
        WorkspaceState::Ready => VisualState::Positive,
        WorkspaceState::Missing | WorkspaceState::NotObserved => VisualState::Attention,
        // 見に行く対象がないことは、良し悪しではない。
        WorkspaceState::NotApplicable => VisualState::Neutral,
    };
    Inline::state(state.as_str(), visual)
}
