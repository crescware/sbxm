use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::{Fact, ProgressSink, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use crate::support::provisioning::ProvisioningState;
use crate::support::select::ProjectPrompt;
use crate::support::{generation, image, provisioning, secret};

use super::PrepareOutput;

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
    let mut locked =
        crate::support::select::one(location, requested, &msg!("select-prepare-heading"), prompt)?
            .lock()?;
    generation::require_no_rebuild(&locked.metadata)?;

    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let layout = SandboxLayout::new(&canonical);
    let observed = provisioning::observe(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &layout,
        workspace_root,
        false,
    )?;
    match observed.state {
        ProvisioningState::Pending => return Err(pending(&locked.metadata)),
        ProvisioningState::Incomplete => {
            return Err(incomplete(&locked.metadata, &observed.artifacts));
        }
        ProvisioningState::Fresh | ProvisioningState::Ready => {}
    }

    let mut warnings = image::cleanup_stale_archives(&locked.paths)?;

    if let Some(mut output) = provisioning::already_built(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &layout,
        workspace_root,
    )? {
        output.warnings = warnings;
        // 全成果物が揃ったことをここで証明できた。直前のintent消去だけが中断で
        // 残っていても、この経路で確実に消し、Readyとの矛盾を残さない。
        provisioning::clear_intent(&mut locked)?;
        return Ok(output);
    }

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。
    secret::require_github(host, name.as_str())?;
    crate::support::docker::require_reachable(host)?;

    // 中断後もbaseと同じgeneration選択規則で続行する。intentがある場合も、既にimageが
    // 完成していればDockerfile変更のwarningを従来どおり出す。
    let (target, target_warnings) =
        provisioning::fresh_target(host, &locked.paths, &name, &locked.metadata)?;
    warnings.extend(target_warnings);

    provisioning::provision(
        &mut locked,
        config,
        &target,
        host,
        workspace_root,
        progress,
        warnings,
    )
}

fn pending(metadata: &ProjectMetadata) -> Error {
    let target = metadata
        .initial_provisioning
        .as_ref()
        .map_or("<unobserved>", |intent| {
            intent.target_dockerfile_sha256.as_str()
        });
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningPending,
            msg!(
                "error-initial-provisioning-pending",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::sandbox(&metadata.sandbox_name().to_string()))
        .fact(Fact::value(target))
        .remediation(
            Remediation::text(msg!("remediation-initial-provisioning-pending"))
                .try_run(format!("sbxm repair {}", metadata.display_id())),
        ),
    )
}

fn incomplete(metadata: &ProjectMetadata, artifacts: &[provisioning::Artifact]) -> Error {
    let values = artifacts
        .iter()
        .map(provisioning::Artifact::as_str)
        .collect::<Vec<_>>();
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningIncomplete,
            msg!(
                "error-initial-provisioning-incomplete",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::paths(&values))
        .remediation(
            Remediation::text(msg!("remediation-initial-provisioning-incomplete"))
                .try_run(format!("sbxm repair {}", metadata.display_id())),
        ),
    )
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
