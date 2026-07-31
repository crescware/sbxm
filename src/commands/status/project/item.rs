use super::Value;

/// 1件の項目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 項目名のFTL message ID。
    pub item: &'static str,
    pub value: Value,
}
