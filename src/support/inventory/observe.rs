use std::path::Path;

use crate::boundary::host::protocol::SandboxEntry;
use crate::diagnostics::Result;
use crate::metadata::{self};
use crate::paths::ProjectPaths;
use crate::registry::RegistryEntry;

use super::{Observed, state_of};

/// 1件のregistry entryが指す成果物と、復旧待ちかどうかを観測する。
///
/// entryを黙って削除せず、観測できた事実をそのまま返す。metadataが読めないことと
/// 一致しないことは、どちらもentryを信用できない状態として`inconsistent`とする。
///
/// 初回構築のintentが残っているかは、metadataを読むだけで確実に言える。一覧は案件
/// ごとにSandboxの中まで観測しないため、安価に証明できるこの事実だけを添える。
pub(super) fn observe(
    paths: &ProjectPaths,
    entry: &RegistryEntry,
    sandboxes: &[SandboxEntry],
    workspace_root: &Path,
) -> Result<(Observed, bool)> {
    if !paths.root().is_dir() {
        return Ok((Observed::Missing, false));
    }
    let metadata = match metadata::load(paths) {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Ok((Observed::Incomplete, false)),
        Err(_) => return Ok((Observed::Inconsistent, false)),
    };
    if !metadata.repository.same_target(entry.repository()) {
        return Ok((Observed::Inconsistent, false));
    }
    let recovery_pending = metadata.initial_provisioning.is_some();
    Ok((
        Observed::Registered(state_of(sandboxes, &metadata, workspace_root)?),
        recovery_pending,
    ))
}
