use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata, RebuildIntent};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxName};

use crate::design::{ProgressSink, Remediation, Warning};
use crate::support::daemon;
use crate::support::inventory::{self, Poll};
use crate::support::protection::{self, ProtectionConfirmation};
use crate::support::{image, template};

use crate::commands::rebuild::RebuildOutput;

use super::{Prepared, Switch, observe_protection};

/// 明示確認を経てSandboxを作り直す。
///
/// confirmationを得たあとの時間のかかる生成を行い、project lockとexclusive session
/// leaseを保持したまま観測を取り直し、`confirmation`が確認した状態とまだ一致している
/// 場合だけ実際に切り替える。`RebuildIntent`は許可証の発行後、切り替えの直前にだけ
/// commitする。
pub fn execute(
    host: &dyn HostEnvironment,
    mut prepared: Prepared,
    confirmation: ProtectionConfirmation,
    config: &GlobalConfig,
    workspace_root: &Path,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<RebuildOutput> {
    let built = prepare_generation(
        host,
        &prepared.locked.paths,
        &prepared.name,
        &prepared.locked.metadata,
        &prepared.target,
        &prepared.current,
        progress,
    )?;

    let entries = daemon::list(host)?;
    let state = inventory::state_of(&entries, &prepared.locked.metadata, workspace_root)?;
    let existed = state != inventory::ProjectState::NotCreated;
    let current_snapshot = observe_protection(
        host,
        &prepared.locked.metadata,
        &prepared.name,
        state,
        workspace_root,
        poll,
        progress,
    )?;
    let permit = protection::gate::authorize(confirmation, current_snapshot)?;

    if prepared.locked.metadata.rebuild.is_none() {
        prepared.locked.metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: prepared.target.clone(),
            previous_dockerfile_sha256: prepared
                .locked
                .metadata
                .provisioning
                .dockerfile_sha256
                .clone(),
        });
        metadata::update(&prepared.locked.paths, &prepared.locked.metadata)?;
    }

    let mut warnings = built.warnings;
    if prepared.current != prepared.target {
        // 注意だけを出して終えない。現在のDockerfileを適用する手順まで示す。
        warnings.push(
            Warning::text(msg!(
                "warning-dockerfile-changed-during-rebuild",
                project = prepared.locked.metadata.display_id()
            ))
            .explain(msg!("guidance-apply-current-dockerfile"))
            .try_run(format!(
                "sbxm rebuild {}",
                prepared.locked.metadata.display_id()
            )),
        );
    }

    let project = ProjectId::parse(&prepared.locked.metadata.display_id())?;
    let switch = Switch {
        config,
        paths: &prepared.locked.paths,
        project: &project,
        workspace_root,
        poll,
    };
    switch.run(
        host,
        &prepared.name,
        &mut prepared.locked.metadata,
        &built.template,
        existed,
        permit,
        progress,
    )?;

    prepared
        .locked
        .metadata
        .provisioning
        .dockerfile_sha256
        .clone_from(&prepared.target);
    prepared.locked.metadata.rebuild = None;
    metadata::update(&prepared.locked.paths, &prepared.locked.metadata)?;

    Ok(RebuildOutput {
        project: prepared.locked.metadata.display_id(),
        sandbox: prepared.name.as_str().to_string(),
        applied: prepared.target.clone(),
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
    // 中断した再構築を続ける場合、成功済みの工程はinspectしてskipする。
    let template = if let Some(template) = template::existing(host, &built)? {
        template
    } else {
        let archive = image::ensure_archive(host, paths, &built, target, progress)?;
        template::ensure(host, &archive, &built, progress)?
    };
    Ok(Generation {
        template,
        warnings: built.warnings,
    })
}
