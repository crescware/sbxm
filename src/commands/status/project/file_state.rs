/// 宣言file 1件の、hostまたはSandboxでの状態。翻訳しない安定したenum。
///
/// どちらも、sbxmがその配置先へ最後に置いた内容を基準に比べる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// sbxmが最後に置いた内容と同じ。記録が無いSandboxでは、hostのfileと同じ。
    Unchanged,
    /// hostのfileが、sbxmが最後に置いたあとで変わった。次の`apply --files`が置く。
    Updated,
    /// この案件へはまだ置いていない。
    Unplaced,
    /// hostのfileをそのままでは置けない。
    Unreadable,
    /// Sandboxのfileが、sbxmが最後に置いたあとで書き換えられた。
    Modified,
    /// Sandboxのfileはhostと異なり、sbxmが置いた記録も無い。
    Unrecorded,
    /// Sandboxに無い。
    Missing,
    NotObserved,
    NotObservedStopped,
    NotApplicable,
}

impl FileState {
    pub fn as_str(self) -> &'static str {
        match self {
            FileState::Unchanged => "unchanged",
            FileState::Updated => "updated",
            FileState::Unplaced => "unplaced",
            FileState::Unreadable => "unreadable",
            FileState::Modified => "modified",
            FileState::Unrecorded => "unrecorded",
            FileState::Missing => "missing",
            FileState::NotObserved => "not-observed",
            FileState::NotObservedStopped => "not-observed-stopped",
            FileState::NotApplicable => "not-applicable",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            FileState::Unchanged => "legend-file-unchanged",
            FileState::Updated => "legend-file-updated",
            FileState::Unplaced => "legend-file-unplaced",
            FileState::Unreadable => "legend-file-unreadable",
            FileState::Modified => "legend-file-modified",
            FileState::Unrecorded => "legend-file-unrecorded",
            FileState::Missing => "legend-missing",
            FileState::NotObserved => "legend-not-observed",
            FileState::NotObservedStopped => "legend-not-observed-stopped",
            FileState::NotApplicable => "legend-not-applicable",
        }
    }
}
