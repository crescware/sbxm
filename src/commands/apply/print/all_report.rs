use crate::design::{Document, Ui};
use crate::diagnostics::ExitCode;

use crate::commands::apply::AllReport;

use super::all_document;

/// `apply --files --all`の結果を表示する。
///
/// 1件でも適用できなかった案件があればexit code `1`とする。
pub fn all_report(ui: &mut Ui, applied: &AllReport) -> ExitCode {
    ui.stdout(&all_document(applied, ui.locale()));

    let mut diagnostics = Document::new();
    for diagnostic in &applied.failures {
        diagnostics = diagnostics.diagnostic(diagnostic.clone());
    }
    ui.stderr(&diagnostics);

    if applied.failures.is_empty() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

#[cfg(test)]
#[path = "all_report_test.rs"]
mod all_report_test;
