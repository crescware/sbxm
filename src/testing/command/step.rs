use super::End;

/// `ScriptedOs`が1度だけ失敗させられる基本操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// 子を起動する。
    Start,
    /// Ctrl-Cの見張りを置く。
    WatchInterrupts,
    /// 端を待たずに読み書きできるようにする。
    Nonblocking(End),
}
