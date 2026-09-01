use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::SandboxEntry;
use crate::config::GlobalConfig;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::support::{
    daemon, generation, identity, image, inventory, repository, sandbox, secret, template, tools,
};

use super::declared_files::declared_files;
use super::observed_worktrees::observed_worktrees;
use super::{Observation, ProvisioningState};

/// 初回構築の成果物をmutationなしで観測し、共有stateへ分類する。
pub(crate) fn observe(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    config: &GlobalConfig,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<Observation> {
    let current_generation = generation::current_dockerfile_hash(paths)?;
    let stored_generation = metadata.provisioning.dockerfile_sha256.clone();
    let target_generation = metadata.initial_provisioning.as_ref().map_or_else(
        || stored_generation.clone(),
        |intent| intent.target_dockerfile_sha256.clone(),
    );
    let name = SandboxName::derive(metadata.canonical_id());
    let layout = SandboxLayout::new(metadata.canonical_id());
    let mut observation = Observation::new(
        ProvisioningState::Fresh,
        current_generation,
        stored_generation,
        target_generation,
    );

    let entries = daemon::list(host)?;
    let entry = inventory::single(&entries, name.as_str())?;
    if let Some(entry) = entry {
        sandbox::verify_identity(entry, &name, workspace_root)?;
        observation.sandbox_present = true;
        observation.sandbox_state = Some(entry.state);
        // 存在するだけでは安全とみなさない。symlink、他アカウント所有、group/otherへの
        // permissionは`Ready`にも`Incomplete`にも丸めず、ここで拒否する。
        observation.workspace_present =
            sandbox::observe_workspace(workspace_root, &name, true)?.is_matching();
        if observation.workspace_present
            && entry.state == crate::boundary::host::protocol::SandboxState::Running
        {
            observe_sandbox(host, entry, config, metadata, &layout, &mut observation)?;
        }
    } else {
        // Sandboxが無いorphan workspaceは、空であることまで確かめる。中身があると、
        // それがどこから来たかを確認できない。
        observation.workspace_present =
            sandbox::observe_workspace(workspace_root, &name, false)?.is_matching();
    }

    // 完成済みSandboxは、再利用判定にDocker daemonを要しない。Dockerfileが変わった
    // 場合も、既存の成果物をreadyとする事実は変わらず、世代の切替はrebuildの責務である。
    if !observation.is_complete() {
        let stored = generation_artifacts(host, &name, metadata, &observation.stored_generation)?;
        observation.stored_image_present = stored.0;
        observation.stored_image_matches = stored.1;
        observation.stored_template_present = stored.2;
        if observation.current_generation == observation.stored_generation {
            observation.current_image_present = stored.0;
            observation.current_image_matches = stored.1;
            observation.current_template_present = stored.2;
        } else {
            let current =
                generation_artifacts(host, &name, metadata, &observation.current_generation)?;
            observation.current_image_present = current.0;
            observation.current_image_matches = current.1;
            observation.current_template_present = current.2;
        }
    }

    observation.state = if metadata.initial_provisioning.is_some() {
        ProvisioningState::Pending
    } else if observation.is_complete() {
        ProvisioningState::Ready
    } else if observation.has_partial_artifact() {
        ProvisioningState::Incomplete
    } else {
        ProvisioningState::Fresh
    };
    Ok(observation)
}

fn generation_artifacts(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    generation: &str,
) -> Result<(bool, bool, bool)> {
    let image_name = image::image_name(name, generation);
    let Some(identity) = image::inspect(host, &image_name)? else {
        return Ok((false, false, template::find(host, &image_name)?.is_some()));
    };
    let matches = image::labels_match(
        &identity,
        &image::expected_labels(metadata.canonical_id(), generation),
    );
    Ok((true, matches, template::find(host, &image_name)?.is_some()))
}

fn observe_sandbox(
    host: &dyn HostEnvironment,
    entry: &SandboxEntry,
    config: &GlobalConfig,
    metadata: &ProjectMetadata,
    layout: &SandboxLayout,
    observation: &mut Observation,
) -> Result<()> {
    let project = ProjectId::parse(&metadata.display_id())?;
    let sandbox = &entry.name;
    sandbox::require_credentials_isolated(host, sandbox)?;
    observation.credentials_isolated = true;
    observation.secret_present = match secret::require_placeholder_present(host, sandbox) {
        Ok(()) => true,
        Err(error) if error.contains_id(crate::diagnostics::ErrorId::SandboxSecretNotApplied) => {
            false
        }
        Err(error) => return Err(error),
    };
    observation.credential_helper = secret::observe_git_credential(host, sandbox)?;
    observation.files = declared_files(host, sandbox, metadata, config)?;
    observation.files_complete = observation
        .files
        .iter()
        .all(|file| file.placement == crate::support::files::Placement::Unchanged);
    observation.identity_complete = identity::observe(host, sandbox, &metadata.git_identity)?;

    let installed = tools::Installed::observe(host, sandbox)?;
    if installed.has(&tools::Gh) {
        observation.tools_complete = identity::observe_git_protocol(host, sandbox)?;
    } else {
        observation.tools_complete = true;
    }

    let git_dir = layout.bare_git_dir();
    if sandbox::path_exists(host, sandbox, &git_dir)? {
        repository::verify_bare_clone(host, sandbox, &project, &git_dir)?;
        observation.repository_complete = true;
        let mut all_worktrees_exist = true;
        for name in layout.worktree_names(metadata.provisioning.requested_worktrees) {
            if !sandbox::path_exists(host, sandbox, &format!("{}/{name}", layout.bare_root()))? {
                all_worktrees_exist = false;
                break;
            }
        }
        if all_worktrees_exist && metadata.provisioning.start_ref.is_some() {
            observation.worktrees = observed_worktrees(host, sandbox, layout, metadata)?;
            observation.worktrees_complete =
                usize::try_from(metadata.provisioning.requested_worktrees)
                    .is_ok_and(|requested| observation.worktrees.len() == requested);
        }
    }
    Ok(())
}
