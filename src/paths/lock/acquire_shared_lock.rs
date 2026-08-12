use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::diagnostics::Result;
use crate::paths::scope::PathScope;

use super::SharedLock;
use super::acquire_lock::acquire_lock;

/// sharedなOS file lockをtimeout付きで取得する。
///
/// 安全条件は[`acquire_lock`]に従う。同じfileに対する別のshared lockとは共存できるが、
/// exclusive lockとは排他する。
pub fn acquire_shared_lock(
    path: &Path,
    timeout: Duration,
    mode: u32,
    scope: PathScope,
) -> Result<SharedLock> {
    acquire_lock(path, timeout, mode, scope, File::try_lock_shared).map(|file| SharedLock { file })
}
