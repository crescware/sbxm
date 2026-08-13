/// 確認すれば削除してよい、と利用者に判断してもらう対象。
///
/// ここに挙げる情報が指すcommit自体は、`Blocker`の検査でoriginから回収できると
/// 確認済みであり、削除しても失わない。失うのは、削除後に自動では復元できない付随情報
/// （無視対象のpath、ref名、追加remote名、reflogだけに残るcommitの存在、管理外worktreeの
/// 存在、sandboxの書き込み層）だけである。remote URL、Git config値、file内容、credential、
/// secretはどのvariantにも持たせない。
///
/// `worktree`を持つvariantは、その作業ツリー固有の損失だけである。ref、tag、remote、
/// reflogは共有bare repositoryが持つため、どのworktreeの話でもなく、repositoryごとに
/// 1件だけ数える。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmableLoss {
    /// Gitが追跡しない無視対象のpath。
    IgnoredPaths {
        worktree: String,
        paths: Vec<String>,
    },
    /// ローカル所有refの名前（stash、notes、checkoutしていないbranchを含む）。
    LocalRef { reference: String },
    /// branchに設定されたupstream追跡。
    BranchUpstream { branch: String, upstream: String },
    /// tag。
    Tag { name: String },
    /// originとは別の、追加のremote名。
    AdditionalRemote { name: String },
    /// どの参照からも到達できないが、reflogにだけ残るcommitの件数。
    ReflogOnlyCommits { count: u64 },
    /// destroy対象で、project metadataから配置を再現できない作業ツリーの存在。
    UnmanagedWorktree { worktree: String },
    /// sandboxの書き込み層。
    SandboxWritableLayer,
}

impl ConfirmableLoss {
    /// fingerprintの入力に使う、翻訳しない安定表記。
    ///
    /// variantの識別子と、識別に要るfieldだけを並べる。`Debug`表現に依存させると、
    /// 表示や整形の都合でfingerprintが変わる。
    pub(super) fn fingerprint_key(&self) -> String {
        match self {
            ConfirmableLoss::IgnoredPaths { worktree, paths } => {
                let mut sorted = paths.clone();
                sorted.sort();
                format!(
                    "ignored-paths\u{1f}{worktree}\u{1f}{}",
                    sorted.join("\u{1e}")
                )
            }
            ConfirmableLoss::LocalRef { reference } => format!("local-ref\u{1f}{reference}"),
            ConfirmableLoss::BranchUpstream { branch, upstream } => {
                format!("branch-upstream\u{1f}{branch}\u{1f}{upstream}")
            }
            ConfirmableLoss::Tag { name } => format!("tag\u{1f}{name}"),
            ConfirmableLoss::AdditionalRemote { name } => format!("additional-remote\u{1f}{name}"),
            ConfirmableLoss::ReflogOnlyCommits { count } => {
                format!("reflog-only-commits\u{1f}{count}")
            }
            ConfirmableLoss::UnmanagedWorktree { worktree } => {
                format!("unmanaged-worktree\u{1f}{worktree}")
            }
            ConfirmableLoss::SandboxWritableLayer => "sandbox-writable-layer".to_string(),
        }
    }
}
