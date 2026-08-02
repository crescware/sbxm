/// FTL format失敗の理由。localeに依存せず英語で表示する最終手段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatFailureReason {
    UnknownMessage,
    MissingValue,
    Format(String),
}
