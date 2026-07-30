//! `ls`の出力。
//!
//! 一覧そのものが結論であるためsummaryを足さない。管理案件と管理外Sandboxは別のsection
//! とし、管理外が1件もなければそのsectionごと省く。

use crate::i18n::Locale;
use crate::msg;
use crate::ui::{Document, Inline, Table};

use super::super::present::Legend;
use super::run::Listing;

/// `ls`が並べるもの。
pub fn document(listing: &Listing, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);

    let mut projects = Table::new(vec![
        msg!("column-project"),
        msg!("column-sandbox"),
        msg!("column-state"),
    ]);
    for row in &listing.projects {
        projects.push(vec![
            Inline::important(row.project.clone()).into(),
            Inline::text(row.sandbox.clone()).into(),
            legend.project_state(row.state).into(),
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
