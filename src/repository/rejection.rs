use crate::diagnostics::Error;

/// 解釈できなかった理由。
pub(super) enum Rejection {
    /// 形式そのものが受理する2形式ではない。
    Form,
    /// 形式は合っているが、ownerまたはrepositoryが案件IDの規則に違反する。
    Project(Error),
}
