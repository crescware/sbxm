use crate::design::{Inline, VisualState};

use crate::commands::apply::ProjectResult;

/// 全案件への`apply --files`の結果。置かなかった案件は、置けなかった案件と分ける。
pub fn apply_result(result: ProjectResult) -> Inline {
    let visual = match result {
        ProjectResult::Applied => VisualState::Positive,
        ProjectResult::Unchanged | ProjectResult::NotCreated => VisualState::Neutral,
        // 停止中の案件には、宣言がまだ届いていない。
        ProjectResult::Stopped => VisualState::Attention,
        ProjectResult::Failed => VisualState::Negative,
    };
    Inline::state(result.as_str(), visual)
}
