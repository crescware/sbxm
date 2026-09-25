use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;
use crate::paths;
use crate::support::bundle::RefChange;

use crate::commands::fetch::FetchOutput;
use crate::commands::present::Legend;

/// `fetch`が並べるもの。
///
/// 書き換えたrefだけを並べる。退避したrefは、置き換えたrefと同じ行に示す。
pub fn document(output: &FetchOutput, locale: Locale) -> Document {
    let repository = paths::display(&output.repository);
    let Some(changes) = &output.changes else {
        return Document::new().summary(msg!("fetch-nothing", project = output.project.clone()));
    };
    if changes.is_empty() {
        return Document::new().summary(msg!(
            "fetch-unchanged",
            project = output.project.clone(),
            repository = repository
        ));
    }
    let mut legend = Legend::new(locale);
    let mut table = Table::new(vec![
        msg!("column-ref"),
        msg!("column-result"),
        msg!("column-archived"),
    ]);
    for change in changes {
        let archived = match change {
            RefChange::Replaced { archived, .. } | RefChange::Deleted { archived, .. } => {
                archived.clone()
            }
            RefChange::Created { .. } | RefChange::Updated { .. } => String::new(),
        };
        table.push(vec![
            Inline::text(change.reference().to_string()).into(),
            legend.ref_change(change).into(),
            Inline::text(archived).into(),
        ]);
    }
    Document::new()
        .summary(msg!(
            "fetch-done",
            project = output.project.clone(),
            repository = repository,
            namespace = output.namespace.clone()
        ))
        .table(None, table)
        .note(msg!("fetch-branches-untouched"))
        .legend(Legend::heading(), legend.entries())
}
