use crate::diagnostics::Diagnostic;

use crate::support::Row;

use crate::design::Warning;

/// 診断結果。
pub struct GlobalStatus {
    pub rows: Vec<Row>,
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<Warning>,
}

impl GlobalStatus {
    pub fn is_healthy(&self) -> bool {
        self.diagnostics.is_empty()
    }
}
