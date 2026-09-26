use crate::design::{Inline, VisualState};
use crate::support::host_sync::RefChange;

/// Sandboxから取り込んだrefの変化。前の先端を退避した変化は、良し悪しではなく注意である。
pub fn ref_change(change: &RefChange) -> Inline {
    let (value, visual) = match change {
        RefChange::Created { .. } => ("created", VisualState::Positive),
        RefChange::Updated { .. } => ("updated", VisualState::Positive),
        RefChange::Replaced { .. } => ("replaced", VisualState::Attention),
        RefChange::Deleted { .. } => ("deleted", VisualState::Attention),
    };
    Inline::state(value, visual)
}
