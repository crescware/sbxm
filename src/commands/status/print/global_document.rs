use crate::design::{Cell, Document, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::present::Legend;
use crate::commands::status::global::GlobalStatus;

/// global scopeの`status`が並べるもの。
pub fn global_document(status: &GlobalStatus, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);
    let mut table = Table::new(vec![
        msg!("status-column-item"),
        msg!("status-column-status"),
    ]);
    for row in &status.rows {
        let value = legend.global_status(row.status);
        table.push(vec![Cell::label(msg!(row.item)), value.into()]);
    }

    Document::new()
        .table(Some(msg!("status-global-section")), table)
        .legend(Legend::heading(), legend.entries())
}
