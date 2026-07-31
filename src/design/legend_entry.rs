use crate::diagnostics::Msg;

/// 状態値と、その説明。
///
/// 状態値は翻訳しない安定した文字列であり、説明だけを訳す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendEntry {
    pub value: String,
    pub description: Msg,
}

impl LegendEntry {
    pub fn new(value: impl Into<String>, description: Msg) -> LegendEntry {
        LegendEntry {
            value: value.into(),
            description,
        }
    }
}
