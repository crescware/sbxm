use std::path::PathBuf;

use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;

/// 案件のhost側のrepository。Sandboxから保存したcommitはここへ取り込む。
///
/// GitHub repositoryでは、sbxmが案件directoryへ取ったhost cloneである。hostにある
/// repositoryを登録した案件では、登録したその場所である。
pub fn host_repository(paths: &ProjectPaths, metadata: &ProjectMetadata) -> PathBuf {
    metadata
        .repository
        .host_path()
        .map_or_else(|| paths.host_clone(), std::path::Path::to_path_buf)
}
