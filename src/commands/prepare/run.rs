use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::diagnostics::Result;
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::{self, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::design::{Fact, ProgressSink, Warning};
use crate::support::select::ProjectPrompt;
use crate::support::{
    docker, files, generation, identity, image, repository, sandbox, secret, select, template,
    tools,
};

use super::{PrepareOutput, already_built, observed_worktrees};

/// 対象を引数またはpromptで解決し、登録済み案件のSandboxを構築する。
pub fn run(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    prompt: &mut dyn ProjectPrompt,
    progress: &mut dyn ProgressSink,
) -> Result<PrepareOutput> {
    // 対象が決まる前にhostの状態へ触れない。
    let mut locked =
        select::one(location, requested, &msg!("select-prepare-heading"), prompt)?.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;

    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let project = ProjectId::parse(&locked.metadata.display_id())?;

    let layout = SandboxLayout::new(&canonical);
    let mut warnings = Vec::new();

    if let Some(output) = already_built(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &layout,
        workspace_root,
    )? {
        return Ok(output);
    }

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。
    secret::require_github(host, name.as_str())?;

    docker::require_reachable(host)?;

    let current = generation::current_dockerfile_hash(&locked.paths)?;
    let generation = adopt_generation(
        host,
        &locked.paths,
        &mut locked.metadata,
        &name,
        &current,
        &mut warnings,
    )?;

    let built = image::ensure(
        host,
        &name,
        locked.metadata.canonical_id(),
        &locked.paths.dockerfile(),
        &generation,
        progress,
    )?;
    warnings.extend(built.warnings.clone());
    let archive = image::ensure_archive(host, &locked.paths, &built, &generation, progress)?;
    let loaded = template::ensure(host, &archive, &built, progress)?;

    let ready = sandbox::ensure(host, &name, &loaded, workspace_root, progress)?;
    if ready.workspace_restored {
        // 消えていたmount点を作り直したことを、成功のなかへ黙って混ぜない。対象のpathと
        // 変更範囲を示し、Sandboxの中には触れていないことまで告げる。
        warnings.push(
            Warning::text(msg!(
                "warning-workspace-restored",
                sandbox = ready.name.clone()
            ))
            .fact(Fact::path(&paths::display(&ready.workspace)))
            .explain(msg!("guidance-workspace-restored")),
        );
    }
    // hostのSSH Agentが届かないことを、daemonの起動条件から推定せず中から確かめる。
    sandbox::require_credentials_isolated(host, &ready.name)?;
    secret::require_placeholder_present(host, &ready.name)?;

    let files = files::place_all(host, &ready.name, &config.files, files::Conflict::Refuse)?;
    identity::ensure(host, &ready.name, &locked.metadata.git_identity)?;
    tools::SandboxReady::announce(host, &ready.name)?;
    secret::configure_git_credential(host, &ready.name)?;

    repository::ensure_bare_clone(host, &ready.name, &project, &layout, progress)?;
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
    )?;

    let worktrees = observed_worktrees(host, &ready.name, &layout, &locked.metadata)?;

    Ok(PrepareOutput {
        project: locked.metadata.display_id(),
        sandbox: ready.name,
        mode: locked.metadata.provisioning.mode,
        start_ref: branch,
        sandbox_state: ready.state,
        worktrees,
        files,
        already_built: false,
        warnings,
    })
}

/// 初回構築を完成させる世代を決める。
///
/// image buildの前にDockerfileが変わった場合は、現在のDockerfileを目標とする。
/// 既にimageがある場合は保存済み世代で完成させ、現在の内容は`rebuild`へ案内する。
fn adopt_generation(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    metadata: &mut ProjectMetadata,
    name: &SandboxName,
    current: &str,
    warnings: &mut Vec<Warning>,
) -> Result<String> {
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    if current == stored {
        return Ok(stored);
    }

    if image::generation_is_built(host, name, metadata.canonical_id(), &stored)? {
        // 注意だけを出して終えない。現在のDockerfileを適用する手順まで示す。
        warnings.push(
            Warning::text(msg!(
                "warning-dockerfile-changed-during-build",
                project = metadata.display_id()
            ))
            .explain(msg!("guidance-apply-current-dockerfile"))
            .try_run(format!("sbxm rebuild {}", metadata.display_id())),
        );
        return Ok(stored);
    }

    metadata.provisioning.dockerfile_sha256 = current.to_string();
    metadata::update(paths, metadata)?;
    Ok(current.to_string())
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "resume_test.rs"]
mod resume_test;

#[cfg(test)]
#[path = "generation_test.rs"]
mod generation_test;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;

#[cfg(test)]
#[path = "tools_test.rs"]
mod tools_test;

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;
