use std::borrow::Cow;
use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::{SandboxEntry, SandboxState};
use crate::config::GlobalConfig;
use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::Result;
use crate::metadata;
use crate::msg;
use crate::paths;
use crate::support::select::Locked;
use crate::support::{daemon, sandbox};

use super::{
    Observation, ProvisioningInputs, ProvisioningOutput, build_initial, initial_intent, observe,
    provision, provision_interior, verify_external_preconditions,
};

/// 初回構築の中断記録を、観測済みの成果物を保持したまま完成させる。
///
/// `entries`は、`observation`を作ったときのSandboxの一覧である。
pub(crate) fn resume_initial(
    locked: &mut Locked,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    entries: &[SandboxEntry],
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
    if !intent.files.is_empty() {
        progress.step(msg!("progress-status-files"));
    }
    let inputs = ProvisioningInputs::resume(&locked.paths, &intent, needs_dockerfile)?;
    let output = if observation.sandbox.is_matching() {
        let mut warnings = Vec::new();
        // 観測に使った一覧から探す。観測からここまでに、Sandboxのrecordを変えるものは無い。
        let ready =
            sandbox::restore_workspace(entries, &locked.metadata.sandbox_name(), workspace_root)?;
        if ready.workspace_restored {
            warnings.push(
                Warning::text(msg!("warning-workspace-restored", sandbox = ready.name))
                    .fact(Fact::path(&paths::display(&ready.workspace)))
                    .explain(msg!("guidance-workspace-restored")),
            );
        }
        provision_interior(locked, &inputs, host, progress, warnings)?
    } else {
        let preconditions = verify_external_preconditions(host, &locked.metadata, progress)?;
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

    progress.step(msg!("progress-inspect-result"));
    // runningのSandboxの中だけを補った場合、recordは変わっていないため、受け取った
    // 一覧をそのまま使う。`provision`はSandboxを作成し得る。止まっていたSandboxは、
    // 中を進める`sbx exec`が起動する。どちらも、受け取った一覧はもう正しくない。
    let entries = if observation.sandbox.is_matching()
        && observation.sandbox_state == Some(SandboxState::Running)
    {
        Cow::Borrowed(entries)
    } else {
        progress.step(msg!("progress-inspect-sandbox"));
        Cow::Owned(daemon::list(host)?)
    };
    let completed = observe(
        host,
        &entries,
        &locked.paths,
        &locked.metadata,
        workspace_root,
        progress,
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
