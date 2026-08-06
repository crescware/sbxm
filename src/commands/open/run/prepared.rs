use crate::paths::SharedLock;

use super::ClampedIndex;

/// 接続先と、接続前に見せる情報。
#[derive(Debug)]
pub struct Prepared {
    pub project: String,
    pub sandbox: String,
    /// 接続先のSSH host名。
    pub ssh_host: String,
    /// SSH sessionを開始するSandbox内のdirectory。
    pub working_directory: String,
    /// 指定されたindexが見つからず、repository rootへfallbackした場合のindex。
    pub missing_worktree_index: Option<u32>,
    /// promptで確定したindexを、lock済みmetadataの範囲まで下げた場合のその内訳。
    pub clamped_worktree_index: Option<ClampedIndex>,
    pub worktrees: Vec<String>,
    /// project lockが外れたあともSSH sessionの生存中だけ保持するshared session lease。
    ///
    /// 通常rebuild/destroyのexclusive session leaseと排他する。読まれることはなく、
    /// `Prepared`が破棄される（`connect`が戻る）ときにdropだけが意味を持つ。
    pub(super) _session_lease: SharedLock,
}
