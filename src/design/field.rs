use crate::diagnostics::Msg;

use crate::design::text::Inline;

/// 項目名と値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub label: Msg,
    pub value: Inline,
}

impl Field {
    pub fn new(label: Msg, value: Inline) -> Field {
        Field { label, value }
    }
}
