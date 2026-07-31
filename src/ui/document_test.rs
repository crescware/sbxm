use crate::testing::outcome::{Checked, Unmet};

use super::*;

use crate::msg;
use crate::ui::style::VisualState;

#[test]
fn blocks_keep_the_order_they_were_declared_in() {
    let document = Document::new()
        .summary(msg!("add-registered"))
        .fields(
            Some(msg!("status-project-section")),
            vec![Field::new(
                msg!("add-field-project"),
                Inline::important("owner/repository"),
            )],
        )
        .try_command("sbxm prepare owner/repository");

    assert!(matches!(document.blocks()[0], Block::Summary(_)));
    assert!(matches!(document.blocks()[1], Block::Section(_)));
    assert!(matches!(document.blocks()[2], Block::Command(_)));
    assert_eq!(document.blocks().len(), 3);
}

#[test]
fn a_section_without_content_is_left_out() {
    let document = Document::new()
        .fields(Some(msg!("status-project-section")), Vec::new())
        .table(
            Some(msg!("status-worktrees-section")),
            Table::new(vec![msg!("column-path")]),
        )
        .lines(None, Vec::new())
        .legend(msg!("legend-heading"), Vec::new());
    assert!(document.is_empty(), "{:?}", document.blocks());
}

#[test]
fn an_empty_result_can_still_be_shown_when_zero_is_the_answer() {
    let document = Document::new().empty_section(
        Some(msg!("ls-projects-section")),
        msg!("error-no-managed-projects"),
    );
    assert_eq!(document.blocks().len(), 1);
}

#[test]
fn a_command_that_cannot_be_built_leaves_no_block_behind() {
    let document = Document::new().try_command("");
    assert!(document.is_empty());
}

#[test]
fn a_table_section_keeps_the_cells_it_was_given() -> Checked {
    let table = Table::new(vec![msg!("column-project"), msg!("column-state")]).row(vec![
        Inline::text("owner/repository").into(),
        Inline::state("running", VisualState::Positive).into(),
    ]);
    let document = Document::new().table(Some(msg!("ls-projects-section")), table);

    let Block::Section(section) = &document.blocks()[0] else {
        return Err(Unmet::new("a table becomes a section".to_string()));
    };
    let SectionBody::Table(table) = &section.body else {
        return Err(Unmet::new("the body stays a table".to_string()));
    };
    assert_eq!(table.rows().len(), 1);
    assert_eq!(
        table.rows()[0][1],
        Inline::state("running", VisualState::Positive).into()
    );
    Ok(())
}

#[test]
fn guidance_keeps_its_numbering_across_the_command_blocks_between_it() {
    // 番号はcommand行ではなく説明行に付く。commandがblockを分断しても番号は続く。
    let document = Document::new()
        .guidance(
            Some(msg!("add-next-heading")),
            vec![GuidanceItem::Ordered {
                number: 1,
                text: msg!("add-next-secret"),
            }],
        )
        .try_command("sbx secret set EXAMPLE")
        .guidance(
            None,
            vec![GuidanceItem::Ordered {
                number: 2,
                text: msg!("add-next-prepare"),
            }],
        )
        .try_command("sbxm prepare owner/repository");

    let numbers: Vec<usize> = document
        .blocks()
        .iter()
        .filter_map(|block| match block {
            Block::Guidance(guidance) => guidance.items.first(),
            _ => None,
        })
        .filter_map(|item| match item {
            GuidanceItem::Ordered { number, .. } => Some(*number),
            GuidanceItem::Plain(_) => None,
        })
        .collect();
    assert_eq!(numbers, vec![1, 2]);
}

#[test]
fn concatenating_documents_preserves_both_orders() {
    let first = Document::new().summary(msg!("add-registered"));
    let second = Document::new().note(msg!("files-secret-hint"));
    let joined = first.concat(second);

    assert!(matches!(joined.blocks()[0], Block::Summary(_)));
    assert!(matches!(joined.blocks()[1], Block::Note(_)));
}
