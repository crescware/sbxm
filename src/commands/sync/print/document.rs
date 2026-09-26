use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;
use crate::paths;
use crate::support::bundle::{ReflectResult, Reflected};

use crate::commands::present::Legend;
use crate::commands::sync::SyncOutput;

/// `sync`が並べるもの。
///
/// hostのbranchとtagで変わったか断られたものと、Sandboxのorigin側で変わったものを、
/// 別の表に並べる。gitが断ったrefは、commitを残した場所を示す。
pub fn document(output: &SyncOutput, locale: Locale) -> Document {
    let repository = paths::display(&output.repository);
    let reflected: &[Reflected] = output.reflected.as_deref().unwrap_or_default();
    if reflected.is_empty() && output.sent.is_empty() {
        return Document::new().summary(msg!(
            "sync-unchanged",
            project = output.project.clone(),
            repository = repository
        ));
    }

    let mut legend = Legend::new(locale);
    let mut host = Table::new(vec![msg!("column-ref"), msg!("column-result")]);
    for entry in reflected {
        host.push(vec![
            Inline::text(entry.reference.clone()).into(),
            legend.reflect_result(&entry.result).into(),
        ]);
    }
    let mut sandbox = Table::new(vec![msg!("column-ref"), msg!("column-result")]);
    for change in &output.sent {
        sandbox.push(vec![
            Inline::text(change.reference().to_string()).into(),
            legend.sent_change(change).into(),
        ]);
    }

    let mut document = Document::new()
        .summary(msg!(
            "sync-done",
            project = output.project.clone(),
            repository = repository.clone()
        ))
        .table(Some(msg!("sync-heading-host")), host)
        .table(Some(msg!("sync-heading-sandbox")), sandbox);
    // 遅れているだけのbranchは、hostが既にそのcommitを持つ。残した場所を示すのは、Gitが
    // 断ったものがあるときだけである。
    if output.left_as_it_was() {
        document = document.note(msg!(
            "sync-left-as-it-was",
            repository = repository,
            namespace = output.namespace.clone()
        ));
    }
    for entry in reflected {
        if let ReflectResult::Refused { reason } = &entry.result {
            document = document.note(msg!(
                "sync-refused-because",
                reference = entry.reference.clone(),
                reason = reason.clone()
            ));
        }
    }
    document
        .note(msg!("sync-sandbox-untouched"))
        .legend(Legend::heading(), legend.entries())
}
