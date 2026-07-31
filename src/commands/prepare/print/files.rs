use crate::design::{Document, Inline, Table};
use crate::msg;
use crate::paths;
use crate::support::files::PlacedFile;

use crate::commands::present::Legend;

/// 宣言fileの配置結果と、そこへ入れてはいけないものの注記。
///
/// 注記はtableの末尾へ接着させず、独立したblockにする。
pub fn files(placed: &[PlacedFile], legend: &mut Legend) -> Document {
    let mut table = Table::new(vec![
        msg!("column-file"),
        msg!("column-destination"),
        msg!("column-result"),
    ]);
    for file in placed {
        table.push(vec![
            Inline::path(paths::display(&file.source)).into(),
            Inline::path(file.destination.clone()).into(),
            legend.placement(file.placement).into(),
        ]);
    }
    if table.is_empty() {
        return Document::new();
    }
    Document::new()
        .table(None, table)
        .note(msg!("files-secret-hint"))
}
