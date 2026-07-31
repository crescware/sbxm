use super::{Color, StyleSpec, VisualState};

/// 状態値に対応する装飾。neutralは着色しない。
pub fn state_style(state: VisualState) -> StyleSpec {
    match state {
        VisualState::Positive => StyleSpec::color(Color::Green),
        VisualState::Attention => StyleSpec::color(Color::Yellow),
        VisualState::Negative => StyleSpec::color(Color::Red),
        VisualState::Neutral => StyleSpec::plain(),
    }
}
