use crate::design::{Inline, VisualState};

use crate::commands::stop::StopResult;

/// 停止commandの結果。停止できたことは成功である。
pub fn stop_result(result: StopResult) -> Inline {
    let visual = match result {
        StopResult::Stopped => VisualState::Positive,
        StopResult::Unchanged => VisualState::Neutral,
        StopResult::Failed => VisualState::Negative,
    };
    Inline::state(result.as_str(), visual)
}
