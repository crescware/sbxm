/// 操作の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// 描き直して続ける。
    Continue,
    /// 確定した候補のindex。
    Done(Vec<usize>),
    /// 何も変更せず終える。
    Canceled,
}
