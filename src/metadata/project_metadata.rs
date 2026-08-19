use crate::project::{CanonicalProjectId, SandboxName};
use crate::repository::RepositoryIdentity;

use super::{GitIdentity, InitialProvisioningIntent, Provisioning, RebuildIntent};

/// 1案件のmetadata。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMetadata {
    /// 登録対象の不変なrepository identity。
    ///
    /// clone URL文字列から実行時にtransportを推測し直さないよう、解釈済みの構造で持つ。
    pub repository: RepositoryIdentity,
    pub provisioning: Provisioning,
    /// `Sandbox内で使うGit` identity。登録時のhost設定のsnapshotである。
    pub git_identity: GitIdentity,
    /// 初回構築を明示的な`repair`へ委ねるための永続intent。
    pub initial_provisioning: Option<InitialProvisioningIntent>,
    pub rebuild: Option<RebuildIntent>,
}

impl ProjectMetadata {
    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        self.repository.display_id()
    }

    /// 突き合わせの正本となるcanonical project ID。
    pub fn canonical_id(&self) -> &CanonicalProjectId {
        self.repository.canonical_id()
    }

    /// canonical project `IDから決定的に導出したSandbox名`。
    pub fn sandbox_name(&self) -> SandboxName {
        SandboxName::derive(self.canonical_id())
    }
}
