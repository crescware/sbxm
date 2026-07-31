use crate::design::{Inline, VisualState};
use crate::support::inventory::Observed;

use super::project_state;

/// registry entryから観測した案件の状態。
///
/// registryと成果物が食い違っている状態は、注意ではなく失敗として示す。
pub fn observed(observed: &Observed) -> Inline {
    match observed {
        Observed::Registered(state) => project_state(*state),
        Observed::Incomplete => Inline::state(observed.as_str(), VisualState::Attention),
        Observed::Missing | Observed::Inconsistent => {
            Inline::state(observed.as_str(), VisualState::Negative)
        }
    }
}
