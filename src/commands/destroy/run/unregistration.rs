use crate::paths::ProjectPaths;
use crate::project::CanonicalProjectId;
use crate::repository::RepositoryIdentity;

/// この実行が管理を解いた案件。registry entryを外す条件の観測に使う。
///
/// project lockを手放したあとで判定するため、canonical project IDだけでなく、
/// destroyがcommitした状態を確かめるのに要る情報をまとめて持つ。
#[derive(Debug, Clone)]
pub struct Unregistration {
    pub(super) paths: ProjectPaths,
    pub(super) repository: RepositoryIdentity,
}

impl Unregistration {
    pub(crate) fn canonical_id(&self) -> &CanonicalProjectId {
        self.repository.canonical_id()
    }
}
