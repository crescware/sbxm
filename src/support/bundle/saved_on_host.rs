use crate::boundary::host::HostEnvironment;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::support::repository;

use super::{SavedTip, saved_tips};

/// 案件のhost側のrepositoryへ保存済みの先端。読めなければ空とする。
///
/// 読めなかった先端から辿れることは確かめられない。空として扱えば、保護は拒否する
/// 側へ倒れる。
pub fn saved_on_host(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
) -> Vec<SavedTip> {
    saved_tips(
        host,
        &repository::host_repository(paths, metadata),
        &metadata.sandbox_name(),
    )
    .unwrap_or_default()
}
