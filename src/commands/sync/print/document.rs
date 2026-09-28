use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;
use crate::paths;
use crate::support::host_sync::{ReflectResult, Reflected};

use crate::commands::present::Legend;
use crate::commands::sync::{SentChange, SyncOutput};

/// `sync`が並べるもの。
///
/// hostのbranchとtagで変わったか断られたものと、Sandboxのorigin側で変わったものを、
/// 別の表に並べる。gitが断ったrefは、commitを残した場所を示す。
///
/// 要約で同期したと言うのは、`sync`が動かせるものがすべて動いたときだけである。断られた
/// refがあれば、終了statusと同じく、同期しきれなかったことを示す。Sandboxのbranchは
/// `sync`が動かさないため、hostのcommitが足りないbranchを名指しして、取り込み方を示す。
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

    let headline = if output.refused_any() {
        Document::new().warning(
            msg!(
                "sync-partly",
                project = output.project.clone(),
                repository = repository.clone()
            ),
            Vec::new(),
        )
    } else {
        Document::new().summary(msg!(
            "sync-done",
            project = output.project.clone(),
            repository = repository.clone()
        ))
    };
    let mut document = headline
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
        match &entry.result {
            ReflectResult::Refused { reason } => {
                document = document.note(msg!(
                    "sync-refused-because",
                    reference = entry.reference.clone(),
                    reason = reason.clone()
                ));
            }
            ReflectResult::LocalChanges { worktree, paths } => {
                document = document.note(msg!(
                    "sync-local-changes",
                    reference = entry.reference.clone(),
                    worktree = paths::display(worktree),
                    paths = paths.join(", ")
                ));
            }
            _ => {}
        }
    }
    for change in &output.sent {
        if let SentChange::Refused { reference, reason } = change {
            document = document.note(msg!(
                "sync-refused-because",
                reference = reference.clone(),
                reason = reason.clone()
            ));
        }
    }
    let behind = branches(reflected, &ReflectResult::Behind);
    if !behind.is_empty() {
        document = document.note(msg!("sync-sandbox-behind", branches = behind));
    }
    let diverged = branches(reflected, &ReflectResult::Diverged);
    if !diverged.is_empty() {
        document = document.note(msg!("sync-sandbox-diverged", branches = diverged));
    }
    document.legend(Legend::heading(), legend.entries())
}

/// 反映の結果が`result`だったbranchの名前を、並べて1つの文字列にする。
fn branches(reflected: &[Reflected], result: &ReflectResult) -> String {
    reflected
        .iter()
        .filter(|entry| &entry.result == result)
        .map(|entry| {
            entry
                .reference
                .strip_prefix("refs/heads/")
                .unwrap_or(&entry.reference)
        })
        .collect::<Vec<_>>()
        .join(", ")
}
