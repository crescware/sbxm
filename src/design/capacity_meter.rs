use crate::diagnostics::Msg;

use super::{Field, Inline, VisualState};

/// 残っている容量と割合を示す、静的なゲージ。
///
/// callerは意味と値だけを渡す。文字集合、色、桁揃えはrendererが決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityMeter {
    pub target: Inline,
    pub available: Field,
    pub remaining: Msg,
    pub percent: u8,
    pub state: VisualState,
    pub status: Option<Msg>,
    pub total: Field,
}
