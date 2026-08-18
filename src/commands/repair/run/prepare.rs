use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::{Fact, Inline, Warning};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use crate::support::provisioning::{self, Artifact, Observation, ProvisioningState};
use crate::support::select::{self, Locked, ProjectPrompt};

use super::{Prepared, RepairPlan};

/// repairのread-only診断を行い、必要ならexclusive session leaseを保持した計画を返す。
pub fn prepare(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
) -> Result<Prepared> {
    let locked =
        select::one(location, requested, &msg!("select-repair-heading"), prompt)?.lock()?;
    if locked.metadata.rebuild.is_some() {
        crate::support::generation::require_no_rebuild(&locked.metadata)?;
    }

    let observation = observe(host, &locked, workspace_root)?;
    match observation.state {
        ProvisioningState::Fresh => Ok(Prepared::Fresh {
            project: locked.metadata.display_id(),
        }),
        ProvisioningState::Ready => {
            let Some(output) = observation.output else {
                return Err(incomplete(&locked.metadata, &observation.artifacts));
            };
            Ok(Prepared::Healthy { output })
        }
        ProvisioningState::Pending | ProvisioningState::Incomplete => {
            let (target, warnings) = target_for(host, &locked)?;
            let session_lease = locked.acquire_exclusive_session_lease()?;
            // project lockとsession leaseを取った後に、成果物を再観測する。
            let latest = observe(host, &locked, workspace_root)?;
            match latest.state {
                ProvisioningState::Fresh => Ok(Prepared::Fresh {
                    project: locked.metadata.display_id(),
                }),
                ProvisioningState::Ready => {
                    let Some(output) = latest.output else {
                        return Err(incomplete(&locked.metadata, &latest.artifacts));
                    };
                    Ok(Prepared::Healthy { output })
                }
                ProvisioningState::Pending | ProvisioningState::Incomplete => {
                    let has_sandbox = latest
                        .artifacts
                        .iter()
                        .any(|artifact| matches!(artifact, Artifact::Sandbox));
                    provisioning::preflight(
                        &locked,
                        config,
                        &target,
                        host,
                        workspace_root,
                        has_sandbox,
                        false,
                    )?;
                    require_neutral_workspace(&locked, &latest, workspace_root)?;
                    Ok(Prepared::Repairable(Box::new(RepairPlan {
                        project: locked.metadata.display_id(),
                        sandbox: locked.metadata.sandbox_name().to_string(),
                        target_generation: target,
                        artifacts: artifact_names(&latest),
                        warnings,
                        locked,
                        session_lease,
                    })))
                }
            }
        }
    }
}

fn observe(
    host: &dyn HostEnvironment,
    locked: &Locked,
    workspace_root: &Path,
) -> Result<Observation> {
    let name = locked.metadata.sandbox_name();
    let layout = SandboxLayout::new(locked.metadata.canonical_id());
    provisioning::observe(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &layout,
        workspace_root,
        true,
    )
}

fn require_neutral_workspace(
    locked: &Locked,
    observation: &Observation,
    workspace_root: &Path,
) -> Result<()> {
    let has_sandbox = observation
        .artifacts
        .iter()
        .any(|artifact| matches!(artifact, Artifact::Sandbox));
    let has_workspace = observation
        .artifacts
        .iter()
        .any(|artifact| matches!(artifact, Artifact::Workspace));
    if has_sandbox || !has_workspace {
        return Ok(());
    }

    let name = locked.metadata.sandbox_name();
    if crate::support::sandbox::workspace_is_empty(workspace_root, &name)? {
        return Ok(());
    }
    let path = crate::support::sandbox::workspace_path(workspace_root, &name);
    Err(Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningIncomplete,
            msg!(
                "error-initial-provisioning-incomplete",
                project = locked.metadata.display_id()
            ),
        )
        .fact(Fact::path(&crate::paths::display(&path)))
        .remediation(msg!("remediation-initial-provisioning-incomplete")),
    ))
}

fn target_for(host: &dyn HostEnvironment, locked: &Locked) -> Result<(String, Vec<Warning>)> {
    let metadata = &locked.metadata;
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    let current = crate::support::generation::current_dockerfile_hash(&locked.paths)?;
    let Some(intent) = &metadata.initial_provisioning else {
        if current == stored {
            return Ok((stored, Vec::new()));
        }

        // intent導入前の途中状態では、metadataと現在のDockerfileだけではtargetを
        // 決めない。完全一致を検証できたimageが片方だけにある場合だけ、その世代を
        // recovery targetとして採用する。
        let name = SandboxName::derive(metadata.canonical_id());
        let stored_built = crate::support::image::generation_is_built(
            host,
            &name,
            metadata.canonical_id(),
            &stored,
        )?;
        let current_built = crate::support::image::generation_is_built(
            host,
            &name,
            metadata.canonical_id(),
            &current,
        )?;
        return match (stored_built, current_built) {
            (true, false) => Ok((
                stored,
                vec![crate::support::provisioning::changed_dockerfile_warning(
                    metadata,
                )],
            )),
            (false, true) => Ok((current, Vec::new())),
            _ => Err(target_unresolved(metadata, &stored, &current)),
        };
    };

    let target = {
        if intent.target_dockerfile_sha256 != metadata.provisioning.dockerfile_sha256 {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::InitialProvisioningInvalid,
                    msg!(
                        "error-initial-provisioning-invalid",
                        project = metadata.display_id()
                    ),
                )
                .fact(crate::design::Fact::value(&intent.target_dockerfile_sha256))
                .remediation(msg!("remediation-initial-provisioning-invalid")),
            ));
        }
        intent.target_dockerfile_sha256.clone()
    };
    if current == target {
        return Ok((target, Vec::new()));
    }

    let name = SandboxName::derive(metadata.canonical_id());
    if crate::support::image::generation_is_built(host, &name, metadata.canonical_id(), &target)? {
        return Ok((
            target,
            vec![crate::support::provisioning::changed_dockerfile_warning(
                metadata,
            )],
        ));
    }
    Ok((target, Vec::new()))
}

fn target_unresolved(metadata: &ProjectMetadata, stored: &str, current: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningIncomplete,
            msg!(
                "error-initial-provisioning-incomplete",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::new(
            crate::diagnostics::Msg::new("diagnostic-value-label"),
            Inline::important(format!("metadata target: {stored}")),
        ))
        .fact(Fact::new(
            crate::diagnostics::Msg::new("diagnostic-value-label"),
            Inline::important(format!("current Dockerfile: {current}")),
        ))
        .remediation(msg!("remediation-initial-provisioning-incomplete")),
    )
}

fn artifact_names(observation: &Observation) -> Vec<String> {
    if observation.artifacts.is_empty() {
        return vec!["initial provisioning metadata".to_string()];
    }
    observation.artifacts.iter().map(Artifact::as_str).collect()
}

fn incomplete(metadata: &ProjectMetadata, artifacts: &[Artifact]) -> Error {
    let values = artifacts.iter().map(Artifact::as_str).collect::<Vec<_>>();
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningIncomplete,
            msg!(
                "error-initial-provisioning-incomplete",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::paths(&values))
        .remediation(msg!("remediation-initial-provisioning-incomplete")),
    )
}

#[cfg(test)]
#[path = "prepare_test.rs"]
mod prepare_test;
