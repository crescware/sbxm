use crate::diagnostics::{Error, ErrorId};
use crate::msg;

/// promptが候補に対応しない選択を返した場合。cancelとは区別する。
pub fn unresolved(index: usize, count: usize) -> Error {
    Error::new(
        ErrorId::SelectionUnresolved,
        msg!("error-selection-unresolved", index = index, count = count),
    )
}
