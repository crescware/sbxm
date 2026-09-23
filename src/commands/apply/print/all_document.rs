use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::apply::{AllReport, ProjectResult};
use crate::commands::present::Legend;

/// `apply --files --all`が並べるもの。
pub fn all_document(applied: &AllReport, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);
    let mut table = Table::new(vec![
        msg!("column-project"),
        msg!("column-sandbox"),
        msg!("column-result"),
    ]);
    for outcome in &applied.outcomes {
        table.push(vec![
            Inline::important(outcome.project.clone()).into(),
            Inline::text(outcome.sandbox.clone()).into(),
            legend.apply_result(outcome.result).into(),
        ]);
    }
    let mut document = Document::new().table(None, table);
    // 停止中のSandboxには宣言がまだ届いていない。黙って残さず、届ける手順を示す。
    if applied
        .outcomes
        .iter()
        .any(|outcome| outcome.result == ProjectResult::Stopped)
    {
        document = document
            .note(msg!("apply-all-stopped-note"))
            .try_command("sbxm apply <project-id> --files");
    }
    document.legend(Legend::heading(), legend.entries())
}
