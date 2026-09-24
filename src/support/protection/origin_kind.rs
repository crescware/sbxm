use crate::metadata::ProjectMetadata;
use crate::repository::Provider;

/// 保護の検査が、commitを回収できる先として読むもの。
///
/// 拒否の説明と対処は、読んだ先によって変わる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OriginKind {
    /// Sandboxのoriginが指すremote。GitHubの案件がこれにあたる。
    Remote,
    /// 案件を登録したhostのrepository。
    Host,
}

impl OriginKind {
    pub(super) fn of(metadata: &ProjectMetadata) -> OriginKind {
        match metadata.repository.provider() {
            Provider::Github => OriginKind::Remote,
            Provider::Local => OriginKind::Host,
        }
    }
}
