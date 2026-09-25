use crate::design::{Document, Ui};
use crate::diagnostics::ExitCode;

use crate::commands::stop::StopReport;

use super::document;

/// `stop`の結果を表示する。
///
/// 1件でも失敗していればexit code `1`とする。
pub fn report(ui: &mut Ui, stopped: &StopReport) -> ExitCode {
    // 保存は止める前に1件ずつ行った。保存の結果は、止めた結果の前にまとめて示す。
    // 保存の進捗と、止めるあいだの外部toolの出力は、実行の途中で既に端末へ出ている。
    for saved in &stopped.saved {
        crate::commands::saving::auto_saved(ui, saved);
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
