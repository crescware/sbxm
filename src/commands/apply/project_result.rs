/// `apply --files --all`での1案件の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectResult {
    /// 宣言fileを1件以上Sandboxへ書き込んだ。
    Applied,
    /// 宣言fileはすべてSandboxに同じ内容で既にあった。
    Unchanged,
    /// Sandboxが停止していたため、起動せず何も置かなかった。
    Stopped,
    /// Sandboxがまだ無い。初回構築が現在の宣言を置く。
    NotCreated,
    /// 適用できなかった。理由は診断が示す。
    Failed,
}

impl ProjectResult {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectResult::Applied => "applied",
            ProjectResult::Unchanged => "unchanged",
            ProjectResult::Stopped => "stopped",
            ProjectResult::NotCreated => "not-created",
            ProjectResult::Failed => "failed",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            ProjectResult::Applied => "legend-apply-applied",
            ProjectResult::Unchanged => "legend-apply-unchanged",
            ProjectResult::Stopped => "legend-apply-stopped",
            ProjectResult::NotCreated => "legend-apply-not-created",
            ProjectResult::Failed => "legend-apply-failed",
        }
    }
}
