use crate::commands::repair::run::RepairPlan;
use crate::design::{Document, Field, Inline};
use crate::hash::short_hex;
use crate::msg;

/// mutationの前にrepairのtargetと変更範囲を表示する。
pub fn plan_document(plan: &RepairPlan) -> Document {
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
                    msg!("repair-plan-state"),
                    Inline::important(plan.state.to_string()),
                ),
                Field::new(
                    msg!("repair-plan-target-generation"),
                    Inline::text(short_hex(&plan.target_generation)),
                ),
            ],
        )
        .lines(
            Some(msg!("repair-plan-actions-heading")),
            plan.actions.clone(),
        )
}
