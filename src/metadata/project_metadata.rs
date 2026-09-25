use crate::project::{CanonicalProjectId, SandboxName};
use crate::repository::RepositoryIdentity;

use super::{
    GitIdentity, InitialProvisioningFile, InitialProvisioningIntent, Provisioning, RebuildIntent,
};

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
    /// 初回構築の復旧先と、global configの入力snapshot。
    pub initial_provisioning: Option<InitialProvisioningIntent>,
    /// 初回構築が完成した時点で配置済みだった宣言fileのbaseline。
    ///
    /// `repair`はこのbaselineだけを復旧対象にし、現在のglobal configとの差分は`apply`の
    /// 責務とする。この記録が無い案件は、この機能より前に完成した案件である。
    pub declared_files: Option<Vec<InitialProvisioningFile>>,
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

    /// 一度でも構築を完成させた案件か。
    ///
    /// 宣言fileの基準は、構築が完成したときに初めて記録し、以後は消さない。消えた
    /// Sandboxを作り直す構築と、登録したばかりの案件の初回構築を、これで見分ける。
    pub fn was_built(&self) -> bool {
        self.declared_files.is_some()
    }
}
