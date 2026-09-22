use crate::design::{Document, Field, GuidanceItem, Inline};
use crate::msg;

use super::super::GuideOutput;

pub fn document(output: &GuideOutput) -> Document {
    Document::new()
        .summary(msg!(
            "guide-credential-rotation-summary",
            project = output.project
        ))
        .fields(
            None,
            vec![
                Field::new(
                    msg!("guide-field-project"),
                    Inline::important(output.project.clone()),
                ),
                Field::new(
                    msg!("guide-field-sandbox"),
                    Inline::important(output.sandbox.clone()),
                ),
            ],
        )
        .guidance(
            Some(msg!("guide-next-heading")),
            vec![
                GuidanceItem::Ordered {
                    number: 1,
                    text: msg!("guide-credential-rotation-issue"),
                },
                GuidanceItem::Ordered {
                    number: 2,
                    text: msg!("guide-credential-rotation-register"),
                },
            ],
        )
        .try_command(output.register_command.clone())
        .guidance(
            None,
            vec![
                GuidanceItem::Plain(msg!("guide-credential-boundary")),
                GuidanceItem::Ordered {
                    number: 3,
                    text: msg!("guide-credential-rotation-verify"),
                },
                GuidanceItem::Ordered {
                    number: 4,
                    text: msg!("guide-credential-rotation-revoke"),
                },
            ],
        )
}
