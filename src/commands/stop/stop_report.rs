use crate::diagnostics::Diagnostic;

use super::StopOutcome;

/// `stop`の結果。
#[derive(Debug, Clone)]
pub struct StopReport {
    pub outcomes: Vec<StopOutcome>,
    /// 失敗した対象の診断。1件でもあればexit code `1`とする。
    pub failures: Vec<Diagnostic>,
}
