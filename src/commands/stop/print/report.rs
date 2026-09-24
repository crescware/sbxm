use crate::design::{Document, Ui};
use crate::diagnostics::ExitCode;

use crate::commands::stop::StopReport;

use super::document;

/// `stop`の結果を表示する。
///
/// 1件でも失敗していればexit code `1`とする。
pub fn report(ui: &mut Ui, stopped: &StopReport) -> ExitCode {
    // 保存は止める前に行った。結果もその順に示す。
    for saved in &stopped.saved {
        crate::commands::fetch::print::auto_saved(ui, saved);
    }
    ui.stdout(&document(stopped, ui.locale()));

    let mut diagnostics = Document::new();
    for diagnostic in &stopped.failures {
        diagnostics = diagnostics.diagnostic(diagnostic.clone());
    }
    ui.stderr(&diagnostics);

    if stopped.failures.is_empty() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

#[cfg(test)]
#[path = "report_test.rs"]
mod report_test;
