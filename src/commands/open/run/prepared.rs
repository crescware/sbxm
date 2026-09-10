use crate::design::Warning;
use crate::paths::SharedLock;
use crate::support::disk::DiskObservation;
use crate::support::provisioning::ProvisioningOutput;

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
    /// SSH接続前に観測した、root filesystemの使用量。
    pub disk: DiskObservation,
    /// この実行が初回構築を行った場合の、その結果。
    ///
    /// 構築済みの案件を開いた場合は`None`である。接続先を見せる前に、何を作ったかを
    /// 同じ実行の中で示す。
    pub provisioned: Option<ProvisioningOutput>,
    /// 接続準備中に復元した外側の状態。
    pub warnings: Vec<Warning>,
    /// project lockが外れたあともSSH sessionの生存中だけ保持するshared session lease。
    ///
    /// 通常rebuild/destroyのexclusive session leaseと排他する。読まれることはなく、
    /// `connect`がSSH childの終了結果を受け取って戻るときにdropされる。
    pub(super) _session_lease: SharedLock,
}
