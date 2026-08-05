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
    /// 受け付けない打鍵。状態を変えない。
    Ignore,
}
