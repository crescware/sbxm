use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;
use crate::paths;
use crate::support::bundle::RefChange;

use crate::commands::present::Legend;

use super::SaveOutput;

/// 保存が並べるもの。
///
/// 書き換えたrefだけを並べる。退避したrefは、置き換えたrefと同じ行に示す。
pub fn saved_document(output: &SaveOutput, locale: Locale) -> Document {
    let repository = paths::display(&output.repository);
    let Some(changes) = &output.changes else {
        return Document::new().summary(msg!("save-nothing", project = output.project.clone()));
    };
    if changes.is_empty() {
        return Document::new().summary(msg!(
            "save-unchanged",
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
            "save-done",
            project = output.project.clone(),
            repository = repository,
            namespace = output.namespace.clone()
        ))
        .table(None, table)
        .note(msg!("save-branches-untouched"))
        .legend(Legend::heading(), legend.entries())
}

#[cfg(test)]
#[path = "saved_document_test.rs"]
mod saved_document_test;
