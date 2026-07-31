/// unmanaged worktreeの扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmanaged {
    /// `destroy`。保存状態を満たせば削除して良い。
    Allowed,
    /// `rebuild`。配置を再現できないため、存在するだけで拒否する。
    Refused,
}
