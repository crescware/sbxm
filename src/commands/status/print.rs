//! `status`の出力。
//!
//! 表そのものが結論であるためsummaryを足さない。診断はstdoutの表へ列を増やさず、
//! stderrのdiagnosticとして出す。

use crate::error::ExitCode;
use crate::i18n::Locale;
use crate::msg;
use crate::ui::{Cell, Document, Field, Inline, Table, Ui};

use super::super::present::Legend;
use super::global::GlobalStatus;
use super::project::ProjectStatus;

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

/// global scopeの`status`。
pub fn global(ui: &mut Ui, status: &GlobalStatus) -> ExitCode {
    ui.stdout(&global_document(status, ui.locale()));

    for warning in &status.warnings {
        ui.warning(warning);
    }
    for diagnostic in &status.diagnostics {
        ui.stderr(&Document::new().diagnostic(diagnostic.clone()));
    }

    if status.is_healthy() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

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

/// project scopeの`status`。
pub fn project(ui: &mut Ui, status: &ProjectStatus) -> ExitCode {
    ui.stdout(&project_document(status, ui.locale()));

    for diagnostic in &status.diagnostics {
        ui.stderr(&Document::new().diagnostic(diagnostic.clone()));
    }
    if status.is_healthy() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}
