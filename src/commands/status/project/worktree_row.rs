use super::Value;
use crate::support::protection::Reachability;

/// worktree 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    /// metadataとの対応。翻訳しない。
    pub kind: &'static str,
    pub mode: Value,
    pub state: Value,
    /// commitがoriginから回収できる根拠。`state`とは別の軸である。
    pub remote: Reachability,
}
