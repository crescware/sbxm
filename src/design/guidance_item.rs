use crate::diagnostics::Msg;

/// guidanceの1行。
///
/// 番号とbulletはこの行に付く。command行へは決して付かない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuidanceItem {
    /// 順序のある操作。
    Ordered { number: usize, text: Msg },
    /// markerを付けない説明。
    Plain(Msg),
}
