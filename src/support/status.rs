//! 診断結果として表へ並べる値。
//!
//! 状態値は翻訳しない安定したenumとし、表示側は凡例で補う。

/// 表示に使う状態値。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StatusValue {
    /// 宣言が無く、defaultで動作している。
    Defaults,
    Ready,
    Missing,
    Error,
    Running,
    Stopped,
}

impl StatusValue {
    pub fn as_str(self) -> &'static str {
        match self {
            StatusValue::Ready => "ready",
            StatusValue::Missing => "missing",
            StatusValue::Defaults => "defaults",
            StatusValue::Error => "error",
            StatusValue::Running => "running",
            StatusValue::Stopped => "stopped",
        }
    }

    /// 凡例に使うFTL message ID。
    pub fn legend_id(self) -> &'static str {
        match self {
            StatusValue::Ready => "legend-ready",
            StatusValue::Missing => "legend-missing",
            StatusValue::Defaults => "legend-defaults",
            StatusValue::Error => "legend-error",
            StatusValue::Running => "legend-running",
            StatusValue::Stopped => "legend-stopped",
        }
    }
}

impl std::fmt::Display for StatusValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 1行分の診断結果。
#[derive(Debug, Clone)]
pub struct Row {
    /// 項目名のFTL message ID。
    pub item: &'static str,
    pub status: StatusValue,
}
