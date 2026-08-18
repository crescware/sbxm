use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::{Fact, ProgressSink, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use crate::support::provisioning::{self, ProvisioningState};
use crate::support::select::ProjectPrompt;

use super::PrepareOutput;

/// 現行の`prepare`入口。fresh案件だけが共有provisioning境界へ進み、途中状態の暗黙再開は
/// `repair`へ委ねる。
#[allow(clippy::too_many_arguments)]
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
    if locked.metadata.rebuild.is_some() {
        crate::support::generation::require_no_rebuild(&locked.metadata)?;
    }
    if locked.metadata.initial_provisioning.is_none() {
        crate::support::docker::require_reachable(host)?;
    }

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
        ProvisioningState::Pending => Err(pending(&locked.metadata)),
        ProvisioningState::Incomplete => Err(incomplete(&locked.metadata, &observed.artifacts)),
        ProvisioningState::Ready => match observed.output {
            Some(output) => Ok(output),
            None => Err(incomplete(&locked.metadata, &observed.artifacts)),
        },
        ProvisioningState::Fresh => {
            let (target, warnings) =
                provisioning::fresh_target(host, &locked.paths, &name, &locked.metadata)?;
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
    }
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
