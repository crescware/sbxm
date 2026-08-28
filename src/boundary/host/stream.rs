/// 子processが持つ2本の出力の、どちらか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stream {
    Stdout,
    Stderr,
}
