use crate::diagnostics::Diagnostic;
use crate::support::host_sync::AutoSaved;

use super::StopOutcome;

/// `stop`の結果。
#[derive(Debug, Clone)]
pub struct StopReport {
    pub outcomes: Vec<StopOutcome>,
    /// 失敗した対象の診断。1件でもあればexit code `1`とする。
    pub failures: Vec<Diagnostic>,
    /// 止める前に、hostにあるrepositoryへ自動で保存した結果。
    pub saved: Vec<AutoSaved>,
}
