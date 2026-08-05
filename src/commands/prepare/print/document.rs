use crate::design::{Document, Field, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::prepare::PrepareOutput;
use crate::commands::present::Legend;

use super::files;

/// `prepare`が並べるもの。
pub fn document(output: &PrepareOutput, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);

    // 既に完了文を持つ場合はそれをsummaryとし、同じ内容を重ねない。
    let summary = if output.already_built {
        msg!("prepare-already-built", project = output.project)
    } else {
        msg!(
            "prepare-done",
            project = output.project,
            sandbox = output.sandbox
        )
    };

    let document = Document::new().summary(summary).fields(
        None,
        vec![
            Field::new(
                msg!("add-field-project"),
                Inline::important(output.project.clone()),
            ),
            Field::new(
                msg!("add-field-sandbox"),
                Inline::important(output.sandbox.clone()),
            ),
            Field::new(
                msg!("add-field-creation-mode"),
                legend.creation_mode(output.mode),
            ),
            Field::new(
                msg!("add-field-start-branch"),
                Inline::text(output.start_ref.clone()),
            ),
            Field::new(
                msg!("add-field-managed-worktrees"),
                Inline::text(output.worktrees.len().to_string()),
            ),
            Field::new(
                msg!("add-field-sandbox-state"),
                legend.sandbox_state(output.sandbox_state),
            ),
        ],
    );

    let mut worktrees = Table::new(vec![
        msg!("column-worktree"),
        msg!("column-created-from"),
        msg!("column-head"),
        msg!("column-mode"),
    ]);
    for worktree in &output.worktrees {
        worktrees.push(vec![
            Inline::path(worktree.path.clone()).into(),
            Inline::text(worktree.created_from.clone()).into(),
            Inline::text(worktree.head.clone().unwrap_or_else(|| "-".to_string())).into(),
            legend.creation_mode(worktree.mode).into(),
        ]);
    }

    document
        .table(Some(msg!("status-worktrees-section")), worktrees)
        .concat(files(&output.files, &mut legend))
        .legend(Legend::heading(), legend.entries())
}

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
