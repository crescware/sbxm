/// Step 1時点の判定で、commitをoriginから回収できると証明できなかった理由。
///
/// `refs/remotes/origin/*`の直接観測に基づく現行互換の判定であり、Step 5（#83）が
/// 権威あるorigin観測へ置き換える。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginRecoveryFailure {
    /// checkout中のbranchにupstreamが設定されていない。
    NoUpstream,
    /// upstreamはあるが、そこへ載っていないcommitを持つ。
    AheadOfUpstream { upstream: String, count: u64 },
    /// detached HEADが、originのどのremote-trackingからも到達できない。
    UnreachableFromOrigin,
}
