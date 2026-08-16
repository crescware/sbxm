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
