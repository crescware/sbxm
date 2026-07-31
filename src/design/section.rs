use crate::diagnostics::Msg;

use super::SectionBody;

/// 見出しと内容。見出しと内容の間には空行を置かない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub heading: Option<Msg>,
    pub body: SectionBody,
}
