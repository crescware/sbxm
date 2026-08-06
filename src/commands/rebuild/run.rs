use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata, RebuildIntent};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use crate::design::{ProgressSink, Remediation, Warning};
use crate::support::image;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::protection::{self, Unmanaged};
use crate::support::{daemon, docker, generation, select, template};

use super::{RebuildOutput, Switch, Target, start_to_read_saved_state};

/// 対象を引数またはpromptで解決し、保存されていない作業がないことを確かめてから、
/// Sandboxを作り直す。
pub fn run(
    selection: Target,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<RebuildOutput> {
    let Target {
        location,
        requested,
        prompt,
    } = selection;
    // 対象が決まる前にhostの状態へ触れない。
    let mut locked =
        select::one(location, requested, &msg!("select-rebuild-heading"), prompt)?.lock()?;
    image::cleanup_stale_archives(&locked.paths);
    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);

    docker::require_reachable(host)?;

    let current = generation::current_dockerfile_hash(&locked.paths)?;
    // この案件のstateだけを、1回の一覧取得から決める。
    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, &locked.metadata, workspace_root)?;

    // intentがある場合は、intentに固定した世代だけを完成させる。
    let target = if let Some(intent) = &locked.metadata.rebuild {
        intent.target_dockerfile_sha256.clone()
    } else {
        require_created(&locked.metadata, state, &name)?;
        start_to_read_saved_state(
            host,
            &locked.metadata,
            &name,
            state == ProjectState::Stopped,
            workspace_root,
            poll,
            progress,
        )?;
        let layout = SandboxLayout::new(&canonical);
        protection::inspect(
            host,
            name.as_str(),
            &layout,
            &locked.metadata,
            Unmanaged::Refused,
        )?;
        current.clone()
    };

    let built = prepare_generation(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &target,
        &current,
        progress,
    )?;
    if locked.metadata.rebuild.is_none() {
        locked.metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: target.clone(),
            previous_dockerfile_sha256: locked.metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&locked.paths, &locked.metadata)?;
    }

    let mut warnings = built.warnings;
    if current != target {
        // 注意だけを出して終えない。現在のDockerfileを適用する手順まで示す。
        warnings.push(
            Warning::text(msg!(
                "warning-dockerfile-changed-during-rebuild",
                project = locked.metadata.display_id()
            ))
            .explain(msg!("guidance-apply-current-dockerfile"))
            .try_run(format!("sbxm rebuild {}", locked.metadata.display_id())),
        );
    }

    let project = ProjectId::parse(&locked.metadata.display_id())?;
    let context = Switch {
        config,
        paths: &locked.paths,
        project: &project,
        workspace_root,
        poll,
    };
    context.run(host, &name, &mut locked.metadata, &built.template, progress)?;

    locked
        .metadata
        .provisioning
        .dockerfile_sha256
        .clone_from(&target);
    locked.metadata.rebuild = None;
    metadata::update(&locked.paths, &locked.metadata)?;

    Ok(RebuildOutput {
        project: locked.metadata.display_id(),
        sandbox: name.as_str().to_string(),
        applied: target,
        warnings,
    })
}

/// 新世代のimage、archive、Template。
struct Generation {
    template: template::LoadedTemplate,
    warnings: Vec<Warning>,
}

/// target世代の成果物を用意する。
///
/// 現在のDockerfileがtarget世代である場合だけ生成でき、異なる場合は既存の成果物が
/// 揃っていることを条件とする。世代を混在させない。
fn prepare_generation(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    target: &str,
    current: &str,
    progress: &mut dyn ProgressSink,
) -> Result<Generation> {
    if current != target
        && !image::generation_is_built(host, name, metadata.canonical_id(), target)?
    {
        // 固定済みtargetの成果物がなく、Dockerfileも別世代であるため再生成できない。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RebuildGenerationMissing,
                msg!(
                    "error-rebuild-generation-missing",
                    project = metadata.display_id(),
                    target = target,
                    observed = current
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-rebuild-generation-missing"))
                    .explain(msg!("remediation-rebuild-generation-missing-destroy"))
                    .try_run(format!("sbxm destroy --force {}", metadata.display_id())),
            ),
        ));
    }

    let built = image::ensure(
        host,
        name,
        metadata.canonical_id(),
        &paths.dockerfile(),
        target,
        progress,
    )?;
    let mut warnings = Vec::new();
    // 中断した再構築を続ける場合、成功済みの工程はinspectしてskipする。
    let template = if let Some(template) = template::existing(host, &built)? {
        template
    } else {
        let archive = image::ensure_archive(host, paths, &built, target, progress)?;
        let outcome = template::ensure(host, archive.path(), &built, progress);
        archive.cleanup_after(outcome, &mut warnings)?
    };
    warnings.extend(built.warnings);
    Ok(Generation { template, warnings })
}

/// `rebuild`は、Sandboxを持つ案件だけを対象とする。
fn require_created(
    metadata: &ProjectMetadata,
    state: ProjectState,
    name: &SandboxName,
) -> Result<()> {
    match state {
        ProjectState::Running | ProjectState::Stopped => Ok(()),
        ProjectState::NotCreated => Err(inventory::not_created(metadata, name.as_str())),
    }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "resume_test.rs"]
mod resume_test;
