//! `destroy`の出力。
//!
//! 破壊操作の確認画面ではgreenを使わない。削除対象の見出しもredではなくboldとし、
//! 保持対象は既定色にする。redを画面全体へ広げるとerrorと見分けにくくなり、消える
//! ものと残るものを落ち着いて比べられなくなる。
//!
//! force modeの通知は結果ではなく注意であるため、warningとしてstderrへ出す。

use crate::i18n::Locale;
use crate::msg;
use crate::ui::{Cell, Document, Field, GuidanceItem, Inline, Table, Warning};

use super::super::present::Legend;
use super::run::{DestroyOutcome, DestroyPlan, Target};

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
                legend.project_state(plan.state),
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

/// force modeが省く検査。結果ではなく注意である。
pub fn force_notice() -> Warning {
    Warning::text(msg!("destroy-force-notice"))
}

/// 削除後の結果。
pub fn outcome_document(outcome: &DestroyOutcome) -> Document {
    Document::new()
        .summary(msg!("destroy-done", project = outcome.project))
        .guidance(
            Some(msg!("destroy-recovery-heading")),
            vec![GuidanceItem::Plain(msg!("destroy-re-register"))],
        )
        .try_command(outcome.re_register.clone())
}

/// 削除対象・保持対象の1行。pathはそのまま、pathで示せない対象は説明を訳す。
fn target(target: &Target) -> Cell {
    match target {
        Target::Path(path) => Inline::path(path.clone()).into(),
        Target::Described(message) => Cell::label(message.clone()),
    }
}
