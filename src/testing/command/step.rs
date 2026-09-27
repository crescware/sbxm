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
    /// PTYの親側を開く。
    OpenController,
    /// PTYの両端をclose-on-execにする。
    CloseOnExec,
    /// PTYの端末側を使えるようにする。
    Grant,
    /// PTYの端末側の鍵を外す。
    UnlockTerminal,
    /// PTYの端末側の名前を尋ねる。
    TerminalName,
    /// PTYの端末側を開く。
    OpenTerminal,
    /// 端末側の設定を読む。
    Settings,
    /// 端末側の設定を書き込む。
    ApplySettings,
    /// 端末側の大きさを決める。
    SetSize,
    /// PTYの親側を待たずに読み書きできるようにする。
    ControllerNonblocking,
    /// 子の1本のstreamへ渡す、端末側の複製。1本目から数える。
    TerminalStdio(u8),
}
