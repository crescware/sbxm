/// 停止理由の表示方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// error ID、原因、外部出力を含めて説明する失敗。
    Error,
    /// 利用者へは短い説明だけを示す、既知の一時的な状態。
    Notice,
}
