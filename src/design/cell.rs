use crate::diagnostics::Msg;

use crate::design::text::Inline;

/// 1 cell。
///
/// 翻訳する項目名と、翻訳しない値を型で分ける。混ぜると、状態値まで訳してしまう経路と、
/// 項目名を原文のまま出す経路の両方が作れてしまう。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    /// 翻訳する項目名。
    Label(Msg),
    /// 翻訳しない値。
    Value(Inline),
}

impl Cell {
    pub fn label(message: Msg) -> Cell {
        Cell::Label(message)
    }
}

impl From<Inline> for Cell {
    fn from(value: Inline) -> Cell {
        Cell::Value(value)
    }
}
