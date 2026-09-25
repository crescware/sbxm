use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;
use crate::paths;

use crate::commands::present::Legend;
use crate::commands::send::SendOutput;

/// `send`が並べるもの。
///
/// Sandboxのorigin側で変わったrefだけを並べる。worktreeへの取り込みは利用者が決める。
pub fn document(output: &SendOutput, locale: Locale) -> Document {
    let repository = paths::display(&output.repository);
    if output.changes.is_empty() {
        return Document::new().summary(msg!(
            "send-unchanged",
            project = output.project.clone(),
            repository = repository
        ));
    }
    let mut legend = Legend::new(locale);
    let mut table = Table::new(vec![msg!("column-ref"), msg!("column-result")]);
    for change in &output.changes {
        table.push(vec![
            Inline::text(change.reference().to_string()).into(),
            legend.sent_change(change).into(),
        ]);
    }
    Document::new()
        .summary(msg!(
            "send-done",
            project = output.project.clone(),
            repository = repository
        ))
        .table(None, table)
        .note(msg!("send-worktrees-untouched"))
        .legend(Legend::heading(), legend.entries())
}
