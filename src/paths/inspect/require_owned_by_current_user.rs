use std::path::Path;

use crate::diagnostics::Result;

use crate::paths::scope::PathScope;

use super::current_user;

/// 現在の利用者が所有していないpathを、内容を変更せず拒否する。
pub fn require_owned_by_current_user(path: &Path, observed: u32, scope: PathScope) -> Result<()> {
    let expected = current_user();
    if observed == expected {
        return Ok(());
    }
    Err(scope.owner_error(path, observed, expected))
}
