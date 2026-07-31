use super::Checked;

/// 拒否されることを前提とした結果の取り出し。
pub trait Refused<T, E> {
    /// 拒否されることを前提にerrorを取り出す。
    fn refused(self) -> Checked<E>;

    /// 何を拒否させたかを添えてerrorを取り出す。
    fn refused_because(self, reason: &str) -> Checked<E>;
}
