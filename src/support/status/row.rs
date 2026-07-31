use super::StatusValue;

/// 1行分の診断結果。
#[derive(Debug, Clone)]
pub struct Row {
    /// 項目名のFTL message ID。
    pub item: &'static str,
    pub status: StatusValue,
}
