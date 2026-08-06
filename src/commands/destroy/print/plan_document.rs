use crate::design::{Document, Field, GuidanceItem, Inline, Table, VisualState};
use crate::i18n::Locale;
use crate::msg;

use crate::commands::destroy::run::DestroyPlan;
use crate::commands::present::Legend;

use super::target;

/// `destroy`が何を消し、何を残すかを削除前に見せる。
pub fn plan_document(plan: &DestroyPlan, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);

    let mut document = Document::new().fields(
        None,
        vec![
            Field::new(
                msg!("add-field-project"),
                Inline::important(plan.project.clone()),
            ),
            Field::new(
                msg!("add-field-sandbox"),
                Inline::important(plan.sandbox.clone()),
            ),
            Field::new(
                msg!("add-field-sandbox-state"),
                // 削除計画のなかでは、稼働しているという事実に良し悪しはない。
                // ここでgreenを出すと、消える対象が健全であることの承認に見える。
                legend.cell(
                    Inline::state(plan.state.as_str(), VisualState::Neutral),
                    plan.state.legend_id(),
                ),
            ),
        ],
    );

    let mut worktrees = Table::new(vec![
        msg!("column-path"),
        msg!("column-kind"),
        msg!("column-mode"),
        msg!("column-branch"),
        msg!("column-head"),
        msg!("column-remote"),
    ]);
    for worktree in &plan.worktrees {
        // 状態値は翻訳しないため、正本locale以外では説明を添える。
        for (value, description) in worktree.legends() {
            legend.add(value, description);
        }
        worktrees.push(vec![
            Inline::path(worktree.relative.clone()).into(),
            Inline::text(worktree.kind.as_str()).into(),
            Inline::text(worktree.mode.as_str()).into(),
            Inline::text(worktree.branch.clone().unwrap_or_else(|| "-".to_string())).into(),
            Inline::text(worktree.head.clone()).into(),
            Inline::text(worktree.remote.as_str()).into(),
        ]);
    }
    document = document.table(Some(msg!("status-worktrees-section")), worktrees);

    document = document.lines(
        Some(msg!("confirmable-losses-heading")),
        plan.confirmable_losses
            .iter()
            .map(crate::commands::present::confirmable_loss)
            .collect(),
    );

    document = document
        .lines(
            Some(msg!("destroy-removes")),
            plan.removes.iter().map(target).collect(),
        )
        .lines(
            Some(msg!("destroy-keeps")),
            plan.keeps.iter().map(target).collect(),
        );

    // 消す前に、消したあと元へ戻す方法まで見せる。
    document
        .guidance(
            Some(msg!("destroy-recovery-heading")),
            vec![GuidanceItem::Plain(msg!("destroy-re-register"))],
        )
        .try_command(plan.re_register.clone())
        .legend(Legend::heading(), legend.entries())
}
