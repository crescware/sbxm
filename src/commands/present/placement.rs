use crate::design::{Inline, VisualState};
use crate::support::files::Placement;

/// 宣言fileの配置結果。
pub fn placement(placement: Placement) -> Inline {
    let visual = match placement {
        Placement::Placed => VisualState::Positive,
        Placement::Unchanged => VisualState::Neutral,
    };
    Inline::state(placement.as_str(), visual)
}
