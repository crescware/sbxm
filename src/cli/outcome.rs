use crate::commands::Command;

/// parse結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// helpをstdoutへ出してexit code `0`。
    Help(String),
    /// version文字列をstdoutへ出してexit code `0`。
    Version(String),
    Run(Command),
}
