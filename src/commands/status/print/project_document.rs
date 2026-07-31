use crate::design::{Document, Field, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::present::Legend;
use crate::commands::status::project::ProjectStatus;

/// project scopeの`status`が並べるもの。
///
/// 指定案件だけを診断し、global環境の検査結果を混ぜない。
pub fn project_document(status: &ProjectStatus, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);

    let mut fields = vec![Field::new(
        msg!("status-item-project"),
        Inline::important(status.project.clone()),
    )];
    fields.extend(
        status
            .items
            .iter()
            .map(|item| Field::new(msg!(item.item), legend.project_status(item.value))),
    );

    let mut worktrees = Table::new(vec![
        msg!("column-path"),
        msg!("column-kind"),
        msg!("column-mode"),
        msg!("column-state"),
    ]);
    for worktree in &status.worktrees {
        worktrees.push(vec![
            Inline::path(worktree.path.clone()).into(),
            Inline::text(worktree.kind).into(),
            legend.project_status(worktree.mode).into(),
            legend.project_status(worktree.state).into(),
        ]);
    }

    let heading = msg!("status-worktrees-section");
    let document = Document::new().fields(Some(msg!("status-project-section")), fields);
    // 「worktreeが1本もない」という観測自体が診断結果であるため、空でもsectionを残す。
    let document = if worktrees.is_empty() {
        document.empty_section(Some(heading), msg!("status-no-worktrees"))
    } else {
        document.table(Some(heading), worktrees)
    };
    document.legend(Legend::heading(), legend.entries())
}
