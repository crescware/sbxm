use std::path::PathBuf;

/// `files pull`がhostの宣言fileをどうしたか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullOutcome {
    /// Sandbox側の内容がhostと同じだった。
    Same,
    /// Sandbox側の内容でhostの宣言fileを置き換えた。
    Adopted(PathBuf),
    /// 利用者がhostの宣言fileを残すと選んだ。
    Kept,
    /// 訊けない実行だったため、採用するかを決めなかった。
    Undecided,
}
