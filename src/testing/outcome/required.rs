use super::Checked;

/// 成立を前提とした値の取り出し。
pub trait Required<T> {
    /// 値が在ることを前提に取り出す。
    fn required(self) -> Checked<T>;

    /// 何を前提としたかを添えて取り出す。
    fn required_because(self, reason: &str) -> Checked<T>;
}
