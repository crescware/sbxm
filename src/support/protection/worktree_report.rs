use super::{Kind, Mode, Reachability};

/// worktree 1件の観測結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeReport {
    /// bare rootからの相対path。
    pub relative: String,
    pub kind: Kind,
    pub mode: Mode,
    pub head: String,
    /// attached modeのbranch名。
    pub branch: Option<String>,
    pub reachability: Reachability,
}

impl WorktreeReport {
    /// この行が使った状態値と、その説明のmessage ID。
    ///
    /// `reachability`は表示用の`display()`を使う。`Unobservable`は理由を括弧内に含む
    /// ため、`as_str()`のままでは実際に表で見せる値と凡例の対応がずれる。
    pub fn legends(&self) -> [(String, &'static str); 3] {
        [
            (self.kind.as_str().to_string(), self.kind.legend_id()),
            (self.mode.as_str().to_string(), self.mode.legend_id()),
            (self.reachability.display(), self.reachability.legend_id()),
        ]
    }
}
