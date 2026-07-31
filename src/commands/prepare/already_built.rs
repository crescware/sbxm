use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::{daemon, sandbox};

use super::{PrepareOutput, observed_worktrees};

/// 目標構成をすべて満たしたSandboxが既にあるか。
///
/// ある場合は副作用なしのno-op成功とする。判定はmetadataの完全性だけで済ませず、
/// Sandbox identityまで確認する。
pub(super) fn already_built(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    layout: &SandboxLayout,
    workspace_root: &Path,
) -> Result<Option<PrepareOutput>> {
    let _ = paths;
    let provisioning = &metadata.provisioning;
    if provisioning.start_ref.is_none() {
        return Ok(None);
    }

    let sandboxes = daemon::list(host)?;
    let Some(entry) = sandboxes
        .into_iter()
        .find(|entry| entry.name == name.as_str())
    else {
        return Ok(None);
    };

    sandbox::verify_identity(&entry, name, workspace_root)?;

    // 要求した本数が揃っているかは、Sandboxの中を見て決める。中を見られない場合は
    // 揃っているとは言えないため、通常の構築経路を通す。
    for name in layout.worktree_names(provisioning.requested_worktrees) {
        let path = format!("{}/{name}", layout.bare_root());
        if !sandbox::path_exists(host, &entry.name, &path)? {
            return Ok(None);
        }
    }

    let worktrees = observed_worktrees(host, &entry.name, layout, metadata)?;
    Ok(Some(PrepareOutput {
        project: metadata.display_id(),
        sandbox: entry.name,
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone().unwrap_or_default(),
        sandbox_state: entry.state,
        worktrees,
        files: Vec::new(),
        notes: Vec::new(),
        already_built: true,
        warnings: Vec::new(),
    }))
}
