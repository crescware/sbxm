use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::{Fact, ProgressSink, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata;
use crate::msg;
use crate::support::provisioning::{self, Observation, ProvisioningState};

use crate::commands::repair::RepairOutput;

use super::Prepared;

/// plan表示後、固定したtargetへ明示的にrepairする。
pub fn execute(
    host: &dyn HostEnvironment,
    mut prepared: Prepared,
    config: &GlobalConfig,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<RepairOutput> {
    let project = prepared.locked.metadata.display_id();
    let sandbox = prepared.locked.metadata.sandbox_name().to_string();

    if prepared.session_lease.is_none() {
        return Ok(RepairOutput {
            project,
            sandbox,
            target_generation: prepared.target,
            changed: false,
            warnings: Vec::new(),
        });
    }

    if let Some(intent) = &prepared.locked.metadata.initial_provisioning {
        provisioning::validate_intent(intent, config, &prepared.locked.metadata.display_id())?;
    }
    let latest = provisioning::observe(
        host,
        &prepared.paths,
        config,
        &prepared.locked.metadata,
        workspace_root,
    )?;
    if latest.state != prepared.observation.state
        || selected_target(
            &latest,
            prepared.locked.metadata.initial_provisioning.is_some(),
        ) != prepared.target
    {
        return Err(state_changed(
            &prepared.locked.metadata,
            prepared.observation.state,
            latest.state,
        ));
    }
    if let Some(intent) = &prepared.locked.metadata.initial_provisioning {
        provisioning::validate_intent(intent, config, &prepared.locked.metadata.display_id())?;
    }
    let mut warnings = std::mem::take(&mut prepared.warnings);
    if latest.is_complete() {
        // 既に全post-conditionが揃っている場合は、rebuildやcredential再設定を呼ばず、
        // intentのclearだけを行う。
        prepared.locked.metadata.initial_provisioning = None;
        metadata::update(&prepared.paths, &prepared.locked.metadata)?;
        return Ok(RepairOutput {
            project,
            sandbox,
            target_generation: prepared.target,
            changed: true,
            warnings,
        });
    }

    let preconditions = prepared
        .preconditions
        .take()
        .ok_or_else(|| state_changed(&prepared.locked.metadata, latest.state, latest.state))?;

    if prepared.locked.metadata.initial_provisioning.is_none() {
        prepared.locked.metadata.initial_provisioning =
            Some(provisioning::initial_intent(config, &prepared.target)?);
        prepared
            .locked
            .metadata
            .provisioning
            .dockerfile_sha256
            .clone_from(&prepared.target);
        metadata::update(&prepared.paths, &prepared.locked.metadata)?;
    }

    warnings.extend(crate::support::image::cleanup_stale_archives(
        &prepared.paths,
    )?);
    let output = provisioning::provision(
        &mut prepared.locked,
        config,
        &prepared.target,
        preconditions,
        host,
        workspace_root,
        progress,
        warnings,
    )?;

    let completed = provisioning::observe(
        host,
        &prepared.paths,
        config,
        &prepared.locked.metadata,
        workspace_root,
    )?;
    if !completed.is_complete() {
        return Err(provisioning::require_repair(
            &prepared.locked.metadata,
            ProvisioningState::Pending,
        ));
    }
    prepared.locked.metadata.initial_provisioning = None;
    metadata::update(&prepared.paths, &prepared.locked.metadata)?;

    Ok(RepairOutput {
        project: output.project,
        sandbox: output.sandbox,
        target_generation: prepared.target,
        changed: true,
        warnings: output.warnings,
    })
}

fn selected_target(observation: &Observation, has_intent: bool) -> String {
    if has_intent {
        return observation.target_generation.clone();
    }
    if observation.current_generation == observation.stored_generation {
        return observation.stored_generation.clone();
    }
    match (
        observation.stored_image_matches,
        observation.current_image_matches,
    ) {
        (true, false) => observation.stored_generation.clone(),
        (false, true) => observation.current_generation.clone(),
        _ => observation.target_generation.clone(),
    }
}

fn state_changed(
    metadata: &crate::metadata::ProjectMetadata,
    before: ProvisioningState,
    after: ProvisioningState,
) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningStateChanged,
            msg!(
                "error-initial-provisioning-state-changed",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-state-changed",
            before = before,
            after = after
        )))
        .remediation(
            Remediation::text(msg!("remediation-run-repair"))
                .try_run(format!("sbxm repair {}", metadata.display_id())),
        ),
    )
}
