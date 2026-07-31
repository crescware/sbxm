use crate::design::{Inline, VisualState};
use crate::support::status::StatusValue;

/// global環境の要件を満たしているか。
pub fn global_status(value: StatusValue) -> Inline {
    let state = match value {
        StatusValue::Ready | StatusValue::Running => VisualState::Positive,
        // 宣言が無いことは、既定で動くという答えそのものである。
        StatusValue::Defaults => VisualState::Neutral,
        // 要件として見た場合、不在も停止も行動を促す。
        StatusValue::Missing | StatusValue::Stopped => VisualState::Attention,
        StatusValue::Error => VisualState::Negative,
    };
    Inline::state(value.as_str(), state)
}
