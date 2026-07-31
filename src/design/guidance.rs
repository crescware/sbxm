use crate::diagnostics::Msg;

use super::GuidanceItem;

/// 補足と次の行動。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guidance {
    pub heading: Option<Msg>,
    pub items: Vec<GuidanceItem>,
}
