use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SandboxState;
use crate::design::{ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::support::select::Locked;
use crate::support::{disk, files, identity, repository, sandbox, secret, tools};

use super::observed_worktrees::observed_worktrees;
use super::{ProvisioningInputs, ProvisioningOutput};

/// 既存Sandboxの中だけを観測可能な不足工程へ進める。
///
/// image、archive、Template、Sandbox作成は既に済んでいるため触れない。中断後の
/// `open`が既存の世代と作業領域を保持したまま再開するための境界でもある。
pub(crate) fn provision_interior(
    locked: &mut Locked,
    inputs: &ProvisioningInputs,
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
    warnings: Vec<Warning>,
) -> Result<ProvisioningOutput> {
    let canonical = locked.metadata.canonical_id().clone();
    let ready_name = SandboxName::derive(&canonical).to_string();
    let project = ProjectId::parse(&locked.metadata.display_id())?;
    let layout = SandboxLayout::new(&canonical);

    sandbox::require_credentials_isolated(host, &ready_name)?;
    secret::require_placeholder_present(host, &ready_name)?;

    let decorate = |error| disk::attach_on_failure(host, &ready_name, SandboxState::Running, error);

    inputs.verify_unchanged()?;
    let placed_files = files::place_all(
        host,
        &ready_name,
        &inputs.file_declarations(),
        files::Conflict::Refuse,
    )
    .map_err(decorate)?;
    identity::ensure(host, &ready_name, &locked.metadata.git_identity).map_err(decorate)?;
    tools::SandboxReady::announce(host, &ready_name).map_err(decorate)?;
    secret::configure_git_credential(host, &ready_name).map_err(decorate)?;

    repository::ensure_bare_clone(host, &ready_name, &project, &layout, progress)
        .map_err(decorate)?;
    let branch = repository::resolve_start_ref(
        host,
        &ready_name,
        &layout,
        &locked.paths,
        &mut locked.metadata,
    )?;
    repository::ensure_worktrees(
        host,
        &ready_name,
        &layout,
        &locked.metadata,
        &branch,
        progress,
    )
    .map_err(decorate)?;

    let worktree_names = layout.worktree_names(locked.metadata.provisioning.requested_worktrees);
    let worktrees = observed_worktrees(
        host,
        &ready_name,
        &layout,
        &locked.metadata,
        &worktree_names,
    )?;
    Ok(ProvisioningOutput {
        project: locked.metadata.display_id(),
        sandbox: ready_name,
        mode: locked.metadata.provisioning.mode,
        start_ref: branch,
        sandbox_state: SandboxState::Running,
        worktrees,
        files: placed_files,
        already_built: false,
        warnings,
    })
}
