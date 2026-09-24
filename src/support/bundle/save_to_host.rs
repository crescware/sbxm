use std::path::Path;
use std::time::SystemTime;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

use super::{
    KEPT_BUNDLES, Receipt, RefChange, import_bundle, prune_bundles, receive_bundle, saved_refs,
    stamp,
};

/// Sandboxの`git_dir`のcommitを、hostの`repository`の`refs/sbx/<sandbox>/`へ保存する。
///
/// bundleは案件の`.sbxm/bundles`へ受け取り、取り込めたかどうかによらず、直近の
/// 数件だけを残す。Sandboxのrefが保存済みのものと同じなら、bundleを運ばずに空の変更を
/// 返す。Sandboxが保存するrefを1つも持たなければ`None`を返す。Sandboxは動いている
/// ことを呼び出し側が確かめておく。
pub fn save_to_host(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    sandbox: &SandboxName,
    git_dir: &str,
    repository: &Path,
) -> Result<Option<Vec<RefChange>>> {
    let saved = saved_refs(host, repository, sandbox.as_str())?;
    let received = match receive_bundle(
        host,
        sandbox.as_str(),
        git_dir,
        &paths.bundles_dir(),
        &stamp(SystemTime::now()),
        &saved,
    )? {
        Receipt::Nothing => return Ok(None),
        Receipt::Unchanged => return Ok(Some(Vec::new())),
        Receipt::Bundle(received) => received,
    };
    let imported = import_bundle(
        host,
        repository,
        &received.path,
        sandbox.as_str(),
        &received.label,
    );
    // 取り込めなかったbundleも数えて整理する。取り込めない状態が続いても溜まらない。
    let pruned = prune_bundles(&paths.bundles_dir(), KEPT_BUNDLES);
    let changes = imported?;
    pruned?;
    Ok(Some(changes))
}
