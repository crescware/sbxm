use std::path::Path;
use std::time::SystemTime;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

use super::{KEPT_BUNDLES, RefChange, import_bundle, prune_bundles, receive_bundle, stamp};

/// Sandboxの`git_dir`のcommitを、hostの`repository`の`refs/sbx/<sandbox>/`へ保存する。
///
/// bundleは案件の`.sbxm/bundles`へ受け取り、取り込んだあとは直近の数件だけを残す。
/// Sandboxが保存するrefを1つも持たなければ`None`を返す。Sandboxは動いていることを
/// 呼び出し側が確かめておく。
pub fn save_to_host(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    sandbox: &SandboxName,
    git_dir: &str,
    repository: &Path,
) -> Result<Option<Vec<RefChange>>> {
    let Some(received) = receive_bundle(
        host,
        sandbox.as_str(),
        git_dir,
        &paths.bundles_dir(),
        &stamp(SystemTime::now()),
    )?
    else {
        return Ok(None);
    };
    let changes = import_bundle(
        host,
        repository,
        &received.path,
        sandbox.as_str(),
        &received.label,
    )?;
    prune_bundles(&paths.bundles_dir(), KEPT_BUNDLES)?;
    Ok(Some(changes))
}
