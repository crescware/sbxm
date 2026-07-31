use crate::diagnostics::Result;
use crate::paths::PathScope;

use super::Presence;

/// symlinkを追跡せずにpathの有無を観測する。
///
/// 不在と、観測できないことを区別する。読めないpathを空いているものとして扱わない。
pub(super) fn observe(path: &std::path::Path) -> Result<Presence> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(Presence::Present),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Presence::Absent),
        Err(error) => Err(PathScope::ProjectPath.unreadable_error(path, &error.to_string())),
    }
}
