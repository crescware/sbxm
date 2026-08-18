use crate::design::{Document, Field, GuidanceItem, Inline, Table};
use crate::i18n::Locale;
use crate::msg;

use super::super::run::{Phase, View};

/// repairの診断計画と結果を、変更対象と根拠が分かる形で表示する。
pub fn document(view: &View, _locale: Locale) -> Document {
    let summary = match view.phase {
        Phase::Plan => msg!("repair-plan"),
        Phase::Fresh => msg!("repair-fresh"),
        Phase::Healthy => msg!("repair-healthy"),
        Phase::Repaired => msg!("repair-done", project = view.project.clone()),
    };
    let mut fields = vec![Field::new(
        msg!("add-field-project"),
        Inline::important(view.project.clone()),
    )];
    if let Some(sandbox) = &view.sandbox {
        fields.push(Field::new(
            msg!("add-field-sandbox"),
            Inline::important(sandbox.clone()),
        ));
    }
    if let Some(target) = &view.target_generation {
        fields.push(Field::new(
            msg!("repair-field-target-generation"),
            Inline::important(target.clone()),
        ));
    }
    let document = Document::new().summary(summary).fields(None, fields);
    let document = if view.artifacts.is_empty() {
        document
    } else {
        let mut table = Table::new(vec![msg!("repair-column-observed-artifact")]);
        for artifact in &view.artifacts {
            table.push(vec![Inline::text(artifact.clone()).into()]);
        }
        document.table(Some(msg!("repair-observed-section")), table)
    };
    let next = match view.phase {
        Phase::Fresh => msg!("repair-fresh-guidance"),
        Phase::Plan => msg!("repair-plan-guidance"),
        Phase::Healthy => msg!("repair-healthy-guidance"),
        Phase::Repaired => msg!("repair-done-guidance"),
    };
    document
        .guidance(
            Some(msg!("add-next-heading")),
            vec![GuidanceItem::Plain(next)],
        )
        .try_command(match view.phase {
            Phase::Fresh | Phase::Healthy | Phase::Repaired => {
                format!("sbxm open {}", view.project)
            }
            Phase::Plan => format!("sbxm repair {}", view.project),
        })
}
