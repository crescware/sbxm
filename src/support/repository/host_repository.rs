use std::path::PathBuf;

use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;

/// 案件のhost側のrepository。Sandboxから保存したcommitはここへ取り込む。
pub fn host_repository(paths: &ProjectPaths, _metadata: &ProjectMetadata) -> PathBuf {
    paths.host_clone()
}
