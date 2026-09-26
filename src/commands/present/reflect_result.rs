use crate::design::{Inline, VisualState};
use crate::support::bundle::ReflectResult;

/// Sandboxのbranchやtagを、hostのbranchやtagへ反映した結果。
///
/// Sandboxが遅れているだけのものは、失うものが無いため注意にしない。gitが断ったものは、
/// 動かなかったことに気付けるよう注意にする。
pub fn reflect_result(result: &ReflectResult) -> Inline {
    let (value, visual) = match result {
        ReflectResult::Created => ("created", VisualState::Positive),
        ReflectResult::Updated => ("updated", VisualState::Positive),
        ReflectResult::Behind => ("behind", VisualState::Neutral),
        ReflectResult::Diverged => ("diverged", VisualState::Attention),
        ReflectResult::CheckedOut => ("checked-out", VisualState::Attention),
        ReflectResult::Exists => ("exists", VisualState::Attention),
        ReflectResult::Refused { .. } => ("refused", VisualState::Attention),
    };
    Inline::state(value, visual)
}
