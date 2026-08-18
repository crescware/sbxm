use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::project::SandboxLayout;

use crate::support::sandbox;

use super::WorktreeRow;

/// metadataが宣言するmanaged worktreeの現在の状態。
pub(crate) fn observed_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
) -> Result<Vec<WorktreeRow>> {
    let provisioning = &metadata.provisioning;
    let names = layout.worktree_names(provisioning.requested_worktrees);
    let created_from = provisioning
        .start_ref
        .as_deref()
        .map(crate::git::origin_ref)
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(names.len());
    for name in names {
        let path = format!("{}/{name}", layout.bare_root());
        let outcome = sandbox::exec(host, sandbox, &["git", "-C", &path, "rev-parse", "HEAD"])?;
        let head = outcome
            .success()
            .then(|| outcome.stdout_text().trim().to_string())
            .filter(|head| !head.is_empty());
        rows.push(WorktreeRow {
            path: name,
            created_from: created_from.clone(),
            head,
            mode: provisioning.mode,
        });
    }
    Ok(rows)
}
