use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::present::Legend;
use crate::commands::stop::StopReport;

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
