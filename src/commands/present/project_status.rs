use crate::design::{Inline, VisualState};

use crate::commands::status::project::Value as ProjectValue;

/// 1案件の項目の観測結果。
pub fn project_status(value: ProjectValue) -> Inline {
    let state = match value {
        ProjectValue::Ready
        | ProjectValue::Running
        | ProjectValue::Clean
        | ProjectValue::NotExposed => VisualState::Positive,
        ProjectValue::Missing
        | ProjectValue::Mismatch
        | ProjectValue::NotObserved
        | ProjectValue::Changed
        | ProjectValue::Stopped
        | ProjectValue::NotCreated
        | ProjectValue::Dirty
        | ProjectValue::NotObservedStopped => VisualState::Attention,
        // hostの鍵で署名できる状態は、注意ではなく失敗として示す。
        ProjectValue::Exposed => VisualState::Negative,
        // 構成の別と、見に行く対象がないことは、良し悪しではない。
        ProjectValue::Attached | ProjectValue::Detached | ProjectValue::NotApplicable => {
            VisualState::Neutral
        }
    };
    Inline::state(value.as_str(), state)
}
