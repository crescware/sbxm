use crate::design::{Document, Field, Inline};
use crate::hash::short_hex;
use crate::msg;

use crate::commands::present;
use crate::commands::rebuild::run::RebuildPlan;

/// `rebuild`が何を作り直し、何を失うかを実行前に見せる。
pub fn plan_document(plan: &RebuildPlan) -> Document {
    Document::new()
        .fields(
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
                    msg!("rebuild-plan-current-generation"),
                    Inline::text(short_hex(&plan.current_generation)),
                ),
                Field::new(
                    msg!("rebuild-plan-target-generation"),
                    Inline::text(short_hex(&plan.target_generation)),
                ),
            ],
        )
        .lines(
            Some(msg!("confirmable-losses-heading")),
            plan.confirmable_losses
                .iter()
                .map(present::confirmable_loss)
                .collect(),
        )
}
