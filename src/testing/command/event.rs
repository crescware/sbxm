use std::path::PathBuf;

use super::{End, Step};

/// `ScriptedOs`へ判断のcodeが呼んだ基本操作。呼ばれた順に残る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 次に起動する子を専用のprocess groupへ置いた。
    OwnGroup,
    /// 子を起動した。
    Started {
        program: String,
        args: Vec<String>,
        directory: Option<PathBuf>,
    },
    /// 待たずに終わったかを尋ねた。
    CheckedExit,
    /// 終わるまで待った。
    WaitedExit,
    /// 子へ終了signalを送った。
    Ended,
    /// 子から端を引き取った。
    Took(End),
    /// 端を待たずに読み書きできるようにした。
    Nonblocking(End),
    /// これらの端を見張って待った。
    Watched(Vec<End>),
    /// 端から読んだ。
    Read(End),
    /// 端へこのbyte数だけ書けた。
    Wrote(End, usize),
    /// 端を閉じた。
    Closed(End),
    /// Ctrl-Cの見張りを置いた。
    WatchingInterrupts,
    /// Ctrl-Cの見張りを外した。
    StoppedWatching,
    /// PTYについての基本操作を呼んだ。
    Pty(Step),
    /// 端末側の設定をrawにした。
    MadeRaw,
}
