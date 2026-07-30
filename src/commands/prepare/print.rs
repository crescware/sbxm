//! `prepare`の出力。
//!
//! 成果を一行のsummaryへ集め、案件のfields、worktree、宣言file、注記、凡例をそれぞれ
//! 独立したsectionにする。

use crate::i18n::Locale;
use crate::msg;
use crate::paths;
use crate::support::files::PlacedFile;
use crate::support::tools::Note;
use crate::ui::{Document, Field, GuidanceItem, Inline, Table};

use super::super::present::Legend;
use super::run::PrepareOutput;

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
        .concat(notes(&output.notes))
        .legend(Legend::heading(), legend.entries())
}

/// 宣言fileの配置結果と、そこへ入れてはいけないものの注記。
///
/// 注記はtableの末尾へ接着させず、独立したblockにする。
pub fn files(placed: &[PlacedFile], legend: &mut Legend) -> Document {
    let mut table = Table::new(vec![
        msg!("column-file"),
        msg!("column-destination"),
        msg!("column-result"),
    ]);
    for file in placed {
        table.push(vec![
            Inline::path(paths::display(&file.source)).into(),
            Inline::path(file.destination.clone()).into(),
            legend.placement(file.placement).into(),
        ]);
    }
    if table.is_empty() {
        return Document::new();
    }
    Document::new()
        .table(None, table)
        .note(msg!("files-secret-hint"))
}

/// Sandboxに入っているtoolが返した案内。
///
/// どのtoolが返したかは印字しない。sbxmが代わりに実行しないことを示す文面そのものが
/// toolを名乗る。実行を求めるcommandは説明から切り離し、独立blockにする。
pub fn notes(notes: &[Note]) -> Document {
    let mut document = Document::new();
    for note in notes {
        document = document
            .lines(
                Some(note.heading.clone()),
                note.items
                    .iter()
                    .map(|item| Inline::path(item).into())
                    .collect(),
            )
            .guidance(None, vec![GuidanceItem::Plain(note.hint.clone())]);
        for command in &note.commands {
            document = document.command(command.clone());
        }
    }
    document
}
