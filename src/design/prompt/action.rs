/// 打鍵が意味する操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Previous,
    Next,
    DecreaseIndex,
    IncreaseIndex,
    Toggle,
    Confirm,
    Cancel,
    /// 絞り込み文字列へ1文字足す。絞り込めないpromptは状態を変えない。
    Type(char),
    /// 絞り込み文字列の末尾1文字を消す。
    Erase,
    /// 受け付けない打鍵。状態を変えない。
    Ignore,
}
