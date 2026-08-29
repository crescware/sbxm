use std::path::Path;

use crate::boundary::host::protocol::SandboxEntry;
use crate::diagnostics::Result;
use crate::metadata::{self};
use crate::paths::ProjectPaths;
use crate::registry::RegistryEntry;

use super::{Observed, state_of};

/// 1件のregistry entryが指す成果物を観測する。
///
/// entryを黙って削除せず、観測できた事実をそのまま返す。metadataが読めないことと
/// 一致しないことは、どちらもentryを信用できない状態として`inconsistent`とする。
pub(super) fn observe(
    paths: &ProjectPaths,
    entry: &RegistryEntry,
    sandboxes: &[SandboxEntry],
    workspace_root: &Path,
) -> Result<Observed> {
    if !paths.root().is_dir() {
        return Ok(Observed::Missing);
    }
    let metadata = match metadata::load(paths) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Ok(Observed::Incomplete),
        Err(_) => return Ok(Observed::Inconsistent),
    };
    if !metadata.repository.same_target(entry.repository()) {
        return Ok(Observed::Inconsistent);
    }
    Ok(Observed::Registered(state_of(
        sandboxes,
        &metadata,
        workspace_root,
    )?))
}
