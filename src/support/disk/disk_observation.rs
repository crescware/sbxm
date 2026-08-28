use crate::boundary::host::protocol::RootDiskUsage;

/// root filesystemの使用量観測。成功値と、観測できなかった理由を型で区別する。
///
/// 観測できなかった場合も`None`へ丸めず、理由ごとに別のvariantとして残す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskObservation {
    /// running中に`df -Pk /`を実行し、解釈できた。
    Observed(RootDiskUsage),
    /// Sandboxが停止中で、観測のために起動しなかった。
    NotObservedStopped,
    /// Sandboxがまだ作成されていない。
    NotObservedNotCreated,
    /// Sandboxの状態そのものを観測できなかった。
    NotObservedMismatch,
    /// `df`のraw終了statusが127で、command not foundとして扱える。
    CommandMissing,
    /// `df`は起動できたが、非ゼロ終了、または出力を解釈できなかった。
    ParseFailed,
}
