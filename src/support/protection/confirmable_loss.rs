/// 確認すれば削除してよい、と利用者に判断してもらう対象。
///
/// ここに挙げる情報が指すcommit自体は、`ProtectionBlocker`の検査でoriginから回収できると
/// 確認済みであり、削除しても失わない。失うのは、削除後に自動では復元できない付随情報
/// （無視対象のpath、ref名、追加remote名、reflogだけに残るcommitの存在、管理外worktreeの
/// 存在、sandboxの書き込み層）だけである。remote URL、Git config値、file内容、credential、
/// secretはどのvariantにも持たせない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmableLoss {
    /// Gitが追跡しない無視対象のpath。
    IgnoredPaths {
        worktree: String,
        paths: Vec<String>,
    },
    /// ローカル所有refの名前（stash、notes、checkoutしていないbranchを含む）。
    LocalRef { worktree: String, reference: String },
    /// branchに設定されたupstream追跡。
    BranchUpstream {
        worktree: String,
        branch: String,
        upstream: String,
    },
    /// tag。
    Tag { worktree: String, name: String },
    /// originとは別の、追加のremote名。
    AdditionalRemote { worktree: String, name: String },
    /// どの参照からも到達できないが、reflogにだけ残るcommitの件数。
    ReflogOnlyCommits { worktree: String, count: u64 },
    /// destroy対象で、project metadataから配置を再現できない作業ツリーの存在。
    UnmanagedWorktree { worktree: String },
    /// sandboxの書き込み層。
    SandboxWritableLayer,
}
