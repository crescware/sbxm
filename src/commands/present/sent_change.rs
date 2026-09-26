use crate::commands::sync::SentChange;
use crate::design::{Inline, VisualState};

/// hostから送ったことで起きたSandboxのorigin側の変化。消えたrefは注意である。
pub fn sent_change(change: &SentChange) -> Inline {
    let (value, visual) = match change {
        SentChange::Created { .. } => ("created", VisualState::Positive),
        SentChange::Updated { .. } => ("updated", VisualState::Positive),
        SentChange::Removed { .. } => ("removed", VisualState::Attention),
        SentChange::Refused { .. } => ("refused", VisualState::Attention),
    };
    Inline::state(value, visual)
}
