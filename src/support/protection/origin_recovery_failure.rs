/// commitをoriginから回収できると証明できなかった理由。
///
/// checkout中branchのupstreamと、detached HEADから`refs/remotes/origin/*`への
/// 到達性だけを見る。stash、tag、notes、未checkoutのbranch、権威あるorigin観測は
/// この判定の対象外である。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginRecoveryFailure {
    /// checkout中のbranchにupstreamが設定されていない。
    NoUpstream,
    /// upstreamはあるが、そこへ載っていないcommitを持つ。
    AheadOfUpstream { upstream: String, count: u64 },
    /// detached HEADが、originのどのremote-trackingからも到達できない。
    UnreachableFromOrigin,
}

impl OriginRecoveryFailure {
    /// fingerprintの入力に使う、翻訳しない安定表記。
    pub(super) fn fingerprint_key(&self) -> String {
        match self {
            OriginRecoveryFailure::NoUpstream => "no-upstream".to_string(),
            OriginRecoveryFailure::AheadOfUpstream { upstream, count } => {
                format!("ahead-of-upstream\u{1e}{upstream}\u{1e}{count}")
            }
            OriginRecoveryFailure::UnreachableFromOrigin => "unreachable-from-origin".to_string(),
        }
    }
}
