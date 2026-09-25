use crate::config::FileDeclaration;
use crate::design::{Document, Inline, Table};
use crate::msg;
use crate::paths;

/// 宣言されたfile。無ければ、足す手順を示す。
pub fn list(files: &[FileDeclaration]) -> Document {
    if files.is_empty() {
        return Document::new()
            .summary(msg!("files-none-declared"))
            .try_command("sbxm files add <path>");
    }
    let mut table = Table::new(vec![msg!("column-file"), msg!("column-destination")]);
    for declared in files {
        table.push(vec![
            Inline::path(paths::display(declared.source.as_path())).into(),
            Inline::path(paths::display(declared.destination.as_path())).into(),
        ]);
    }
    Document::new().table(None, table)
}
