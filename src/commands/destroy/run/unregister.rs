use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::metadata::{self};
use crate::msg;
use crate::paths::{self, PathScope};
use crate::registry::{RegistryEntry, RegistryGuard};

use crate::design::Warning;

use super::Unregistration;

/// 管理解除の最後に、対応するregistry entryだけを削除する。
///
/// registry entryを先に消して案件を発見不能にしない。project lockを手放したあとで
/// 短時間だけregistry lockを取るため、2つのlockを同時には保持しない。
///
/// どちらのlockも持たない隙間に、同じ案件の`add`が登録をやり直していることがある。
/// canonical project IDの一致だけを根拠にentryを消さず、この実行がcommitした管理解除が
/// registry lockの下でも維持されていることを確かめる。再登録を観測したentryは、
/// 消さずに警告として報告する。
pub fn unregister(
    location: &ConfigLocation,
    unregistration: &Unregistration,
) -> Result<Option<Warning>> {
    let mut guard = RegistryGuard::acquire(location)?;
    let canonical = unregistration.canonical_id();
    let Some(entry) = guard.registry().find(canonical) else {
        return Ok(None);
    };
    if !still_unmanaged(entry, unregistration)? {
        return Ok(Some(registered_again(entry)));
    }
    guard.remove(canonical)?;
    Ok(None)
}

/// この実行が解いた管理が、registry lockの下でも維持されているか。
///
/// 別のrootや別のclone URLへ登録し直されたentryは、この実行が消したentryではない。
/// metadataが戻っていれば案件は再び管理下にあり、entryは新しい登録意図を指している。
/// 観測できなければ、維持されていると推測しない。
fn still_unmanaged(entry: &RegistryEntry, unregistration: &Unregistration) -> Result<bool> {
    if entry.project_root() != unregistration.paths.root()
        || !entry.repository().same_target(&unregistration.repository)
    {
        return Ok(false);
    }
    let root = unregistration.paths.root();
    match std::fs::symlink_metadata(root) {
        // rootごと無くなっていれば、再登録は起きていない。
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(error) => return Err(PathScope::ProjectPath.unreadable_error(root, &error.to_string())),
        Ok(_) => {}
    }
    paths::require_owned_directory(root, PathScope::ProjectPath)?;
    Ok(metadata::load(&unregistration.paths)?.is_none())
}

/// 管理を解いているあいだに登録し直された案件を、entryを残したまま報告する。
fn registered_again(entry: &RegistryEntry) -> Warning {
    Warning::text(msg!(
        "warning-project-registered-again",
        project = entry.repository().display_id(),
        path = paths::display(entry.project_root())
    ))
}
