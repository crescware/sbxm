use std::path::PathBuf;

use crate::paths::{ExclusiveLock, ProjectPaths};
use crate::project::SandboxName;

use crate::support::inventory::ProjectState;
use crate::support::protection::ProtectionSnapshot;
use crate::support::select::{self};

use super::{DestroyPlan, Unregistration};

/// lockを保持したまま確認を挟むための状態。
#[derive(Debug)]
pub struct Prepared {
    pub plan: DestroyPlan,
    pub(super) paths: ProjectPaths,
    pub(super) name: SandboxName,
    pub(super) state: ProjectState,
    /// 中立workspace directoryを置くhost側のroot。
    ///
    /// 削除直前の観測は、`prepare`が最初に観測したときと同じrootを見る必要がある。
    /// 両者が別のrootを見ると、同じ状態でもfingerprintが食い違いうる。
    pub(super) workspace_root: PathBuf,
    pub(super) locked: select::Locked,
    /// 最初に観測した状態指紋。`confirm`が明示確認と引き換えに`take`で消費する。
    ///
    /// `--force`の場合だけ`None`のままであり、`confirm`は確認を求めない。Sandboxが
    /// そもそも無い場合も、その「不在」を観測した`ProtectionSnapshot`を持つ。
    pub(super) snapshot: Option<ProtectionSnapshot>,
    /// 最終protection inspectからsandbox remove完了まで保持するexclusive session lease。
    ///
    /// `--force`は保護ゲートと同様にこのleaseも意図的に迂回するため`None`になる。
    /// 読まれることはなく、`Prepared`が破棄されるときのdropだけが意味を持つ。
    pub(super) _session_lease: Option<ExclusiveLock>,
}

impl Prepared {
    /// 管理解除後にregistryから外す案件。
    pub fn unregistration(&self) -> Unregistration {
        Unregistration {
            paths: self.paths.clone(),
            repository: self.locked.metadata.repository.clone(),
        }
    }
}
