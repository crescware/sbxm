use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::{Fact, ProgressSink, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata;
use crate::msg;
use crate::support::provisioning::{self, ProvisioningInputs, ProvisioningState};

use crate::commands::repair::RepairOutput;

use super::Prepared;
use super::actions_for::actions_for;

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

    let latest = provisioning::observe(
        host,
        &prepared.paths,
        config,
        &prepared.locked.metadata,
        workspace_root,
    )?;
    latest.require_safe()?;
    let has_intent = prepared.locked.metadata.initial_provisioning.is_some();
    let target_still_selected =
        provisioning::select_generation(&latest, &prepared.locked.metadata, has_intent)
            .is_ok_and(|generation| generation == prepared.target);
    if latest.state != prepared.observation.state
        || !target_still_selected
        || actions_for(&prepared.locked.metadata, &latest, has_intent) != prepared.plan.actions
    {
        // 表示した対象と、これから実行する対象が一致しない。displayした一覧だけを
        // 実行するという契約を守るため、ここで拒否し、mutationは1つも起こさない。
        return Err(state_changed(
            &prepared.locked.metadata,
            prepared.observation.state,
            latest.state,
        ));
    }
    if has_intent {
        return resume_intent(host, &mut prepared, config, workspace_root, progress);
    }
    let mut warnings = std::mem::take(&mut prepared.warnings);

    let preconditions = prepared
        .preconditions
        .take()
        .ok_or_else(|| state_changed(&prepared.locked.metadata, latest.state, latest.state))?;

    // intentが再現するべき入力を先にsnapshotへ固定し、そのsnapshotのdigestをintentと
    // 比較してから初めてmutationへ進む。検証後に生きている入力を読み直す隙を作らない。
    let inputs = capture_repair_inputs(&prepared, config)?;

    prepared.locked.metadata.initial_provisioning = Some(provisioning::initial_intent(&inputs));
    prepared
        .locked
        .metadata
        .provisioning
        .dockerfile_sha256
        .clone_from(&prepared.target);
    metadata::update(&prepared.paths, &prepared.locked.metadata)?;

    warnings.extend(crate::support::image::cleanup_stale_archives(
        &prepared.paths,
    )?);
    let output = provisioning::provision(
        &mut prepared.locked,
        &inputs,
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
    completed.require_safe()?;
    if !completed.is_complete() {
        return Err(provisioning::require_open(
            &prepared.locked.metadata,
            ProvisioningState::Pending,
        ));
    }
    prepared.locked.metadata.initial_provisioning = None;
    prepared.locked.metadata.declared_files = Some(provisioning::initial_intent(&inputs).files);
    metadata::update(&prepared.paths, &prepared.locked.metadata)?;

    Ok(RepairOutput {
        project: output.project,
        sandbox: output.sandbox,
        target_generation: prepared.target,
        changed: true,
        warnings: output.warnings,
    })
}

fn resume_intent(
    host: &dyn HostEnvironment,
    prepared: &mut Prepared,
    config: &GlobalConfig,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<RepairOutput> {
    let output =
        provisioning::ensure_initial(&mut prepared.locked, config, host, workspace_root, progress)?;
    Ok(RepairOutput {
        project: output.project,
        sandbox: output.sandbox,
        target_generation: prepared.target.clone(),
        changed: true,
        warnings: output.warnings,
    })
}

fn capture_repair_inputs(prepared: &Prepared, config: &GlobalConfig) -> Result<ProvisioningInputs> {
    ProvisioningInputs::capture(&prepared.paths, config, Some(&prepared.target))
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
