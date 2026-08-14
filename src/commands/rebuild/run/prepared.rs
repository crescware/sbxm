use crate::paths::ExclusiveLock;
use crate::project::SandboxName;

use crate::support::select;

use super::RebuildPlan;

/// lockを保持したまま確認と生成を挟むための状態。
#[derive(Debug)]
pub struct Prepared {
    pub plan: RebuildPlan,
    pub(super) name: SandboxName,
    /// 適用先のDockerfile hash。`RebuildIntent`から読んだ場合は再計算しない。
    pub(super) target: String,
    /// 現在のDockerfileのhash。
    pub(super) current: String,
    pub(super) locked: select::Locked,
    /// project lockを保持している間に取る、exclusive session lease。
    ///
    /// 読まれることはなく、`Prepared`が破棄されるときのdropだけが意味を持つ。
    pub(super) _session_lease: ExclusiveLock,
}
