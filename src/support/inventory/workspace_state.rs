/// 中立workspace directoryをhostで実測した結果。
///
/// runtime stateとは直交する事実である。`ProjectState`へvariantを足して1つのenumに
/// 2つの事実を持たせると、`stopped`と「起動できない」を同じ語で示すことになる。
///
/// 値の語彙は`status`の`status-item-workspace`と揃える。同じ事実を、commandごとに
/// 別の語で示さない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceState {
    /// directoryがhostに在る。
    Ready,
    /// 不在を観測した。Sandboxのrecordは在るが、mount元が無い。
    Missing,
    /// 在るともないとも答えられなかった。不在と同一視しない。
    NotObserved,
    /// 対応するSandboxのrecordが無く、mount元として宣言されたdirectoryが無い。
    NotApplicable,
}

impl WorkspaceState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            WorkspaceState::Ready => "ready",
            WorkspaceState::Missing => "missing",
            WorkspaceState::NotObserved => "not-observed",
            WorkspaceState::NotApplicable => "not-applicable",
        }
    }

    /// 凡例に使うFTL message ID。
    pub fn legend_id(self) -> &'static str {
        match self {
            WorkspaceState::Ready => "legend-ready",
            WorkspaceState::Missing => "legend-missing",
            WorkspaceState::NotObserved => "legend-not-observed",
            WorkspaceState::NotApplicable => "legend-not-applicable",
        }
    }
}
