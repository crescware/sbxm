use crate::design::{Document, GuidanceItem, Inline};
use crate::support::tools::Note;

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
