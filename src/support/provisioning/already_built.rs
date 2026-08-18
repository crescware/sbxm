use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::{daemon, inventory, sandbox};

use super::{ProvisioningOutput, observed_worktrees};

/// 目標構成をすべて満たしたSandboxが既にあるか。
pub(crate) fn already_built(
    host: &dyn HostEnvironment,
    _paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    layout: &SandboxLayout,
    workspace_root: &Path,
) -> Result<Option<ProvisioningOutput>> {
    let provisioning = &metadata.provisioning;
    if provisioning.start_ref.is_none() {
        return Ok(None);
    }

    let sandboxes = daemon::list(host)?;
    let Some(entry) = inventory::single(&sandboxes, name.as_str())? else {
        return Ok(None);
    };

    sandbox::verify_identity(entry, name, workspace_root)?;
    if !sandbox::workspace_exists(workspace_root, name)? {
        return Ok(None);
    }
    for name in layout.worktree_names(provisioning.requested_worktrees) {
        let path = format!("{}/{name}", layout.bare_root());
        if !sandbox::path_exists(host, &entry.name, &path)? {
            return Ok(None);
        }
    }

    let worktrees = observed_worktrees(host, &entry.name, layout, metadata)?;
    Ok(Some(ProvisioningOutput {
        project: metadata.display_id(),
        sandbox: entry.name.clone(),
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone().unwrap_or_default(),
        sandbox_state: entry.state,
        worktrees,
        files: Vec::new(),
        already_built: true,
        warnings: Vec::new(),
    }))
}
