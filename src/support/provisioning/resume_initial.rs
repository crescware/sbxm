use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::metadata;
use crate::msg;
use crate::paths;
use crate::support::sandbox;
use crate::support::select::Locked;

use super::{
    Observation, ProvisioningInputs, ProvisioningOutput, build_initial, initial_intent, observe,
    provision, provision_interior, verify_external_preconditions,
};

/// 初回構築の中断記録を、観測済みの成果物を保持したまま完成させる。
pub(crate) fn resume_initial(
    locked: &mut Locked,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
    observation: &Observation,
) -> Result<ProvisioningOutput> {
    let no_target_artifact = !observation.stored_image_present
        && !observation.stored_template_present
        && observation.sandbox.is_missing();
    if no_target_artifact && observation.current_generation != observation.target_generation {
        // 元の世代へ結び付く成果物が一つも無い場合だけ、修正後の入力へ対象を更新する。
        return build_initial(locked, config, host, workspace_root, progress, None);
    }

    let Some(intent) = locked.metadata.initial_provisioning.clone() else {
        return build_initial(locked, config, host, workspace_root, progress, None);
    };
    if observation.is_complete() {
        locked.metadata.declared_files = Some(intent.files);
        locked.metadata.initial_provisioning = None;
        metadata::update(&locked.paths, &locked.metadata)?;
        return Ok(super::ready_output(&locked.metadata, observation));
    }
    let needs_dockerfile = !observation.sandbox.is_matching() && !observation.stored_image_matches;
    let inputs = ProvisioningInputs::resume(&locked.paths, &intent, needs_dockerfile)?;
    let output = if observation.sandbox.is_matching() {
        let mut warnings = Vec::new();
        let ready =
            sandbox::restore_workspace(host, &locked.metadata.sandbox_name(), workspace_root)?;
        if ready.workspace_restored {
            warnings.push(
                Warning::text(msg!("warning-workspace-restored", sandbox = ready.name))
                    .fact(Fact::path(&paths::display(&ready.workspace)))
                    .explain(msg!("guidance-workspace-restored")),
            );
        }
        provision_interior(locked, &inputs, host, progress, warnings)?
    } else {
        let preconditions = verify_external_preconditions(host, &locked.metadata.sandbox_name())?;
        provision(
            locked,
            &inputs,
            preconditions,
            host,
            workspace_root,
            progress,
            Vec::new(),
        )?
    };

    let completed = observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;
    completed.require_safe()?;
    if !completed.is_complete() {
        return Err(super::require_open(
            &locked.metadata,
            super::ProvisioningState::Pending,
        ));
    }
    locked.metadata.declared_files = Some(initial_intent(&inputs).files);
    locked.metadata.initial_provisioning = None;
    metadata::update(&locked.paths, &locked.metadata)?;
    Ok(output)
}
