use crate::design::{Cell, Inline};

use crate::commands::destroy::run::Target;

/// 削除対象・保持対象の1行。pathはそのまま、pathで示せない対象は説明を訳す。
pub(super) fn target(target: &Target) -> Cell {
    match target {
        Target::Path(path) => Inline::path(path.clone()).into(),
        Target::Described(message) => Cell::label(message.clone()),
    }
}
