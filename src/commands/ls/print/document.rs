use crate::design::{Document, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::ls::Listing;
use crate::commands::present::Legend;

/// `ls`が並べるもの。
pub fn document(listing: &Listing, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);

    let mut projects = Table::new(vec![
        msg!("column-project"),
        msg!("column-project-root"),
        msg!("column-sandbox"),
        msg!("column-state"),
    ]);
    for row in &listing.projects {
        projects.push(vec![
            Inline::important(row.project.clone()).into(),
            Inline::path(row.root.clone()).into(),
            Inline::text(row.sandbox.clone()).into(),
            legend.observed(&row.observed).into(),
        ]);
    }

    let mut unmanaged = Table::new(vec![
        msg!("column-sandbox"),
        msg!("column-state"),
        msg!("column-workspace"),
    ]);
    for row in &listing.unmanaged {
        // 管理外Sandboxの状態はruntimeが返した原文であり、sbxmのenumではない。
        unmanaged.push(vec![
            Inline::text(row.sandbox.clone()).into(),
            Inline::text(row.state.clone()).into(),
            Inline::path(row.workspace.clone()).into(),
        ]);
    }

    let heading = msg!("ls-projects-section");
    // 管理案件がゼロであること自体が答えであるため、空でも一行で示す。
    let document = if projects.is_empty() {
        Document::new().empty_section(Some(heading), msg!("error-no-managed-projects"))
    } else {
        Document::new().table(Some(heading), projects)
    };
    document
        .table(Some(msg!("ls-unmanaged-section")), unmanaged)
        .legend(Legend::heading(), legend.entries())
}
