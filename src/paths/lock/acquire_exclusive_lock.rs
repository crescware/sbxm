use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::diagnostics::Result;
use crate::paths::scope::PathScope;

use super::ExclusiveLock;
use super::acquire_lock::acquire_lock;

/// exclusiveなOS file lockをtimeout付きで取得する。
///
/// 安全条件は[`acquire_lock`]に従う。同じfileに対して、ほかのexclusive lockはもちろん、
/// shared lockとも排他する。
pub fn acquire_exclusive_lock(
    path: &Path,
    timeout: Duration,
    mode: u32,
    scope: PathScope,
) -> Result<ExclusiveLock> {
    acquire_lock(path, timeout, mode, scope, File::try_lock).map(|file| ExclusiveLock { file })
}
