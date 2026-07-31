use crate::diagnostics::Msg;

use crate::design::table::Table;

use crate::design::Cell;

use super::{Field, LegendEntry};

/// sectionが並べるもの。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionBody {
    /// 項目名と値の一覧。
    Fields(Vec<Field>),
    /// 列をそろえた一覧。
    Table(Table),
    /// 1行1件の並び。翻訳する説明と翻訳しない値が混在し得る。
    Lines(Vec<Cell>),
    /// 出現した状態値とその説明。
    Legend(Vec<LegendEntry>),
    /// 対象がゼロであること自体が結果である場合の一行。
    Empty(Msg),
}
