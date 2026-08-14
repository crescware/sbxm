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
    pub fn legends(&self) -> [(&'static str, &'static str); 3] {
        [
            (self.kind.as_str(), self.kind.legend_id()),
            (self.mode.as_str(), self.mode.legend_id()),
            (self.reachability.as_str(), self.reachability.legend_id()),
        ]
    }
}
