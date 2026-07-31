/// 1案件の停止結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopResult {
    /// この実行で停止した。
    Stopped,
    /// この実行では停止していない。
    Unchanged,
    /// 停止に失敗した。
    Failed,
}

impl StopResult {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            StopResult::Stopped => "stopped",
            StopResult::Unchanged => "unchanged",
            StopResult::Failed => "failed",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            StopResult::Stopped => "legend-stopped-now",
            StopResult::Unchanged => "legend-not-stopped",
            StopResult::Failed => "legend-failed",
        }
    }
}
