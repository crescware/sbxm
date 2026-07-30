//! `stop`の出力。
//!
//! 対象ごとの結果の表そのものが結論であるため、summaryを足さない。失敗した対象の診断は
//! 表へ列を増やさず、stderrの別blockとして出す。

use crate::error::ExitCode;
use crate::i18n::Locale;
use crate::msg;
use crate::ui::{Document, Inline, Table, Ui};

use super::super::present::Legend;
use super::run::StopReport;

/// `stop`が並べるもの。
pub fn document(stopped: &StopReport, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);
    let mut table = Table::new(vec![
        msg!("column-project"),
        msg!("column-sandbox"),
        msg!("column-result"),
    ]);
    for outcome in &stopped.outcomes {
        table.push(vec![
            Inline::important(outcome.project.clone()).into(),
            Inline::text(outcome.sandbox.clone()).into(),
            legend.stop_result(outcome.result).into(),
        ]);
    }
    Document::new()
        .table(None, table)
        .legend(Legend::heading(), legend.entries())
}

/// `stop`の結果を表示する。
///
/// 1件でも失敗していればexit code `1`とする。
pub fn report(ui: &mut Ui, stopped: &StopReport) -> ExitCode {
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
