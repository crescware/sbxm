use crate::design::{Document, GuidanceItem};
use crate::msg;

use crate::commands::destroy::run::DestroyOutcome;

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
