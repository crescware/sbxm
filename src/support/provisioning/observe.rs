use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{SandboxEntry, SandboxState};
use crate::config::GlobalConfig;
use crate::diagnostics::{Error, Result};
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use crate::support::Observed;

use crate::support::{
    daemon, generation, identity, image, inventory, repository, sandbox, secret, template, tools,
};

use super::declared_files::declared_files;
use super::observed_worktrees::observed_worktrees;
use super::{Observation, ProvisioningState};

/// 安全と確認できなかった事実を、観測を止めずに集める場所。
type Blocking = Vec<Error>;

/// 初回構築の成果物をmutationなしで観測し、共有stateへ分類する。
///
/// artifact 1件の食い違いや観測失敗で観測全体を打ち切らない。安全でない事実は
/// `Observation`へ記録して最後まで観測を並べ、変更を伴う呼び出し側が
/// `Observation::require_safe`で拒否する。`Err`はhost自体へ問い合わせられなかった
/// 場合だけとする。
pub(crate) fn observe(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    config: &GlobalConfig,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<Observation> {
    let stored_generation = metadata.provisioning.dockerfile_sha256.clone();
    let target_generation = metadata.initial_provisioning.as_ref().map_or_else(
        || stored_generation.clone(),
        |intent| intent.target_dockerfile_sha256.clone(),
    );
    // intentがある再開では保存済み世代が正本である。現在のDockerfileを読めないだけで、
    // 既存成果物やSandboxの完成確認まで拒まない。
    let current_generation = match generation::current_dockerfile_hash(paths) {
        Ok(generation) => generation,
        Err(_) if metadata.initial_provisioning.is_some() => target_generation.clone(),
        Err(error) => return Err(error),
    };
    let name = SandboxName::derive(metadata.canonical_id());
    let layout = SandboxLayout::new(metadata.canonical_id());
    let mut observation = Observation::new(
        ProvisioningState::Fresh,
        current_generation,
        stored_generation,
        target_generation,
    );
    let mut blocking = Blocking::new();

    let entries = daemon::list(host)?;
    let entry = inventory::single(&entries, name.as_str())?;
    if let Some(entry) = entry {
        observation.sandbox_state = Some(entry.state);
        observation.sandbox = match sandbox::verify_identity(entry, &name, workspace_root) {
            Ok(()) => Observed::Matching,
            Err(error) => blocked(&mut blocking, error),
        };
        // 存在するだけでは安全とみなさない。symlink、他アカウント所有、group/otherへの
        // permissionは`Ready`にも`Incomplete`にも丸めず、拒否として記録する。
        observation.workspace = match sandbox::observe_workspace(workspace_root, &name, true) {
            Ok(observed) => observed,
            Err(error) => blocked(&mut blocking, error),
        };
        if observation.sandbox.is_matching()
            && observation.workspace.is_matching()
            && entry.state == SandboxState::Running
        {
            observe_sandbox(
                host,
                entry,
                config,
                metadata,
                &layout,
                &mut observation,
                &mut blocking,
            )?;
        } else {
            // 停止中のSandboxの中は、起動せずには読めない。読まなかったことを欠落と
            // 書かず、観測不能として残す。
            mark_inside_unobservable(&mut observation, entry.state);
        }
    } else {
        // Sandboxが無いorphan workspaceは、空であることまで確かめる。中身があると、
        // それがどこから来たかを確認できない。
        observation.workspace = match sandbox::observe_workspace(workspace_root, &name, false) {
            Ok(observed) => observed,
            Err(error) => blocked(&mut blocking, error),
        };
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

    observation.block_all(blocking);
    observation.state = observation.classify(metadata.initial_provisioning.is_some());
    Ok(observation)
}

/// Sandboxの中を読まなかった区間。欠落ではなく、観測しなかった事実として残す。
fn mark_inside_unobservable(observation: &mut Observation, state: SandboxState) {
    let unobservable = Observed::Unobservable {
        evidence: state.as_str().to_string(),
    };
    observation.files_placed = unobservable.clone();
    observation.identity = unobservable.clone();
    observation.tools = unobservable.clone();
    observation.credentials = unobservable.clone();
    observation.secret = unobservable.clone();
    observation.credential_helper = unobservable.clone();
    observation.repository = unobservable.clone();
    observation.worktrees_present = unobservable;
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

#[allow(clippy::too_many_arguments)]
fn observe_sandbox(
    host: &dyn HostEnvironment,
    entry: &SandboxEntry,
    config: &GlobalConfig,
    metadata: &ProjectMetadata,
    layout: &SandboxLayout,
    observation: &mut Observation,
    blocking: &mut Blocking,
) -> Result<()> {
    let project = ProjectId::parse(&metadata.display_id())?;
    let sandbox = &entry.name;

    observation.credentials = match sandbox::require_credentials_isolated(host, sandbox) {
        Ok(()) => Observed::Matching,
        Err(error) => blocked(blocking, error),
    };
    observation.secret = match secret::require_placeholder_present(host, sandbox) {
        Ok(()) => Observed::Matching,
        Err(error) if error.contains_id(crate::diagnostics::ErrorId::SandboxSecretNotApplied) => {
            Observed::Missing
        }
        Err(error) => blocked(blocking, error),
    };
    observation.credential_helper = match secret::observe_git_credential(host, sandbox) {
        Ok(observed) => observed,
        Err(error) => blocked(blocking, error),
    };
    match declared_files(host, sandbox, metadata, config) {
        Ok(files) => {
            observation.files_placed = if files
                .iter()
                .all(|file| file.placement == crate::support::files::Placement::Unchanged)
            {
                Observed::Matching
            } else {
                Observed::Missing
            };
            observation.files = files;
        }
        Err(error) => observation.files_placed = blocked(blocking, error),
    }
    observation.identity = match identity::observe(host, sandbox, &metadata.git_identity) {
        Ok(true) => Observed::Matching,
        Ok(false) => Observed::Missing,
        Err(error) => blocked(blocking, error),
    };
    observation.tools = observe_tools(host, sandbox, blocking);
    observation.repository = observe_repository(host, sandbox, &project, layout, blocking);
    if observation.repository.is_matching() {
        observe_worktrees(host, sandbox, layout, metadata, observation, blocking);
    }
    Ok(())
}

fn observe_tools(host: &dyn HostEnvironment, sandbox: &str, blocking: &mut Blocking) -> Observed {
    let installed = match tools::Installed::observe(host, sandbox) {
        Ok(installed) => installed,
        Err(error) => return blocked(blocking, error),
    };
    if !installed.has(&tools::Gh) {
        return Observed::Matching;
    }
    match identity::observe_git_protocol(host, sandbox) {
        Ok(true) => Observed::Matching,
        Ok(false) => Observed::Missing,
        Err(error) => blocked(blocking, error),
    }
}

fn observe_repository(
    host: &dyn HostEnvironment,
    sandbox: &str,
    project: &ProjectId,
    layout: &SandboxLayout,
    blocking: &mut Blocking,
) -> Observed {
    let git_dir = layout.bare_git_dir();
    match sandbox::path_exists(host, sandbox, &git_dir) {
        Ok(false) => Observed::Missing,
        Ok(true) => match repository::verify_bare_clone(host, sandbox, project, &git_dir) {
            Ok(()) => Observed::Matching,
            Err(error) => blocked(blocking, error),
        },
        Err(error) => blocked(blocking, error),
    }
}

fn observe_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    observation: &mut Observation,
    blocking: &mut Blocking,
) {
    // 1本でも欠けているからといって、そこで打ち切らない。打ち切ると存在するworktreeの
    // 一覧が空のままになり、`actions_for`が要求本数すべてを作成対象として表示する。
    let mut present = Vec::new();
    let mut all_present = true;
    for name in layout.worktree_names(metadata.provisioning.requested_worktrees) {
        match sandbox::path_exists(host, sandbox, &format!("{}/{name}", layout.bare_root())) {
            Ok(true) => present.push(name),
            Ok(false) => all_present = false,
            Err(error) => {
                observation.worktrees_present = blocked(blocking, error);
                return;
            }
        }
    }
    // 起点branchが決まっていない案件は、worktreeが揃ったとは言えない。それでも、
    // 存在が確認できたものだけはこの下でHEADまで観測する。
    let start_ref_resolved = metadata.provisioning.start_ref.is_some();
    match observed_worktrees(host, sandbox, layout, metadata, &present) {
        Ok(worktrees) => {
            let requested = usize::try_from(metadata.provisioning.requested_worktrees);
            observation.worktrees_present = if all_present
                && start_ref_resolved
                && requested.is_ok_and(|requested| worktrees.len() == requested)
            {
                Observed::Matching
            } else {
                Observed::Missing
            };
            observation.worktrees = worktrees;
        }
        Err(error) => observation.worktrees_present = blocked(blocking, error),
    }
}

/// 安全と確認できなかったerrorを観測結果へ写し、拒否理由として残す。
fn blocked(blocking: &mut Blocking, error: Error) -> Observed {
    // evidenceは翻訳せず、拒否した診断のidをそのまま残す。全文はblockingが持つ。
    let evidence = error
        .diagnostics()
        .first()
        .map_or("unknown", |diagnostic| diagnostic.id.as_str())
        .to_string();
    blocking.push(error);
    Observed::Mismatch { evidence }
}

#[cfg(test)]
#[path = "observe_test.rs"]
mod observe_test;
