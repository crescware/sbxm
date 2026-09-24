use std::path::PathBuf;

use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::repository::Provider;

/// 案件のhost側のrepository。Sandboxから保存したcommitはここへ取り込む。
///
/// GitHub repositoryでは、sbxmが案件directoryへ取ったhost cloneである。hostにある
/// repositoryを登録した案件では、登録したその場所である。
pub fn host_repository(paths: &ProjectPaths, metadata: &ProjectMetadata) -> PathBuf {
    match metadata.repository.provider() {
        Provider::Github => paths.host_clone(),
        Provider::Local => PathBuf::from(metadata.repository.clone_url()),
    }
}
