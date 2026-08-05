/// 操作の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// 描き直して続ける。
    Continue,
    /// 確定した候補のindex。
    Done(Vec<usize>),
    /// `open`で確定した案件とworktree index。
    DoneOpen { project: usize, index: u32 },
    /// 何も変更せず終える。
    Canceled,
}
