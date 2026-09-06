use crate::design::{Document, Field, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use crate::support::provisioning::ProvisioningOutput;

use super::{Legend, placed_files};

/// 初回構築の結果が並べるもの。
///
/// 成果を一行のsummaryへ集め、案件のfields、worktree、宣言file、注記、凡例をそれぞれ
/// 独立したsectionにする。初回構築と復旧のどちらから入っても、同じ結果を同じ語彙で示す。
/// 次の一手は入口ごとに違うため、呼び出し側が足す。
pub fn provisioning_output(output: &ProvisioningOutput, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);

    // 既に完了文を持つ場合はそれをsummaryとし、同じ内容を重ねない。
    let summary = if output.already_built {
        msg!("provisioning-already-built", project = output.project)
    } else {
        msg!(
            "provisioning-done",
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
            Inline::text(worktree.head.clone()).into(),
            legend.creation_mode(worktree.mode).into(),
        ]);
    }

    document
        .table(Some(msg!("status-worktrees-section")), worktrees)
        .concat(placed_files(&output.files, &mut legend))
        .legend(Legend::heading(), legend.entries())
}

#[cfg(test)]
#[path = "provisioning_output_test.rs"]
mod provisioning_output_test;
