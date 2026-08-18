use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::support::select::Locked;
use crate::support::{
    disk, docker, files, identity, image, repository, sandbox, secret, template, tools,
};

use super::{ProvisioningOutput, clear_intent, observed_worktrees, persist_intent};

/// 固定済みgenerationへ向けて初回構築を進める唯一の共有境界。
pub(crate) fn provision(
    locked: &mut Locked,
    config: &GlobalConfig,
    target: &str,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
    mut warnings: Vec<Warning>,
) -> Result<ProvisioningOutput> {
    // secretとengineはread-onlyの事前条件であり、intentより前に確認できる。
    secret::require_github(host, locked.metadata.sandbox_name().as_str())?;
    docker::require_reachable(host)?;

    // これが初回構築における最初のmutationである。
    persist_intent(locked, target)?;

    warnings.extend(image::cleanup_stale_archives(&locked.paths)?);

    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let project = ProjectId::parse(&locked.metadata.display_id())?;
    let layout = SandboxLayout::new(&canonical);
    let generation = locked.metadata.provisioning.dockerfile_sha256.clone();

    let built = image::ensure(
        host,
        &name,
        locked.metadata.canonical_id(),
        &locked.paths.dockerfile(),
        &generation,
        progress,
    )?;
    warnings.extend(built.warnings.clone());
    let loaded = if let Some(loaded) = template::existing(host, &built)? {
        loaded
    } else {
        let archive = image::ensure_archive(host, &locked.paths, &built, &generation, progress)?;
        let outcome = template::ensure(host, archive.path(), &built, progress);
        archive.cleanup_after(outcome, &mut warnings, progress)?
    };

    let ready = sandbox::ensure(host, &name, &loaded, workspace_root, progress)?;
    if ready.workspace_restored {
        warnings.push(
            Warning::text(msg!(
                "warning-workspace-restored",
                sandbox = ready.name.clone()
            ))
            .fact(Fact::path(&paths::display(&ready.workspace)))
            .explain(msg!("guidance-workspace-restored")),
        );
    }
    sandbox::require_credentials_isolated(host, &ready.name)?;
    secret::require_placeholder_present(host, &ready.name)?;

    let decorate = |error| disk::attach_on_failure(host, &ready.name, ready.state, error);

    let placed_files = files::place_all(host, &ready.name, &config.files, files::Conflict::Refuse)
        .map_err(decorate)?;
    identity::ensure(host, &ready.name, &locked.metadata.git_identity).map_err(decorate)?;
    tools::SandboxReady::announce(host, &ready.name).map_err(decorate)?;
    secret::configure_git_credential(host, &ready.name).map_err(decorate)?;

    repository::ensure_bare_clone(host, &ready.name, &project, &layout, progress)
        .map_err(decorate)?;
    let branch = repository::resolve_start_ref(
        host,
        &ready.name,
        &layout,
        &locked.paths,
        &mut locked.metadata,
    )?;
    repository::ensure_worktrees(
        host,
        &ready.name,
        &layout,
        &locked.metadata,
        &branch,
        progress,
    )
    .map_err(decorate)?;

    // ensure_worktreesは各工程のpost-conditionを検査し、ここでは最終表示用の観測を行う。
    let worktrees = observed_worktrees(host, &ready.name, &layout, &locked.metadata)?;
    super::preflight(locked, config, target, host, workspace_root, true, true)?;
    let output = ProvisioningOutput {
        project: locked.metadata.display_id(),
        sandbox: ready.name,
        mode: locked.metadata.provisioning.mode,
        start_ref: branch,
        sandbox_state: ready.state,
        worktrees,
        files: placed_files,
        already_built: false,
        warnings,
    };

    // intentの消去までが構築完了のcommit。失敗すればintentを残したまま返す。
    clear_intent(locked)?;
    Ok(output)
}
