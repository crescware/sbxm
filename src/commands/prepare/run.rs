use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::ProjectId;
use crate::support::provisioning::ProvisioningInputs;
use crate::support::select::ProjectPrompt;
use crate::support::{generation, image, provisioning};

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
    // 対象が決まる前にhostの状態へ触れない。
    let mut locked =
        crate::support::select::one(location, requested, &msg!("select-prepare-heading"), prompt)?
            .lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    let observation = provisioning::observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;

    match observation.state {
        provisioning::ProvisioningState::Ready => {
            return Ok(ready_output(&locked.metadata, &observation));
        }
        provisioning::ProvisioningState::Pending | provisioning::ProvisioningState::Incomplete => {
            return Err(provisioning::require_repair(
                &locked.metadata,
                observation.state,
            ));
        }
        provisioning::ProvisioningState::Fresh => {}
    }

    let name = locked.metadata.sandbox_name();

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。Dockerの到達性も
    // ここで一度だけ確認し、以降の`provision`の中では再確認しない。
    let preconditions = provisioning::verify_external_preconditions(host, &name)?;

    // Dockerfileと宣言fileを1回だけ読み、privateなsnapshotへ複製する。以降はこの
    // snapshotだけを使い、生きているhost pathを二度と読まない。
    let inputs = ProvisioningInputs::capture(&locked.paths, config, None)?;

    // metadataのintentとtarget generationを、最初のhost側mutationより先にatomicに保存する。
    locked.metadata.initial_provisioning = Some(provisioning::initial_intent(&inputs));
    locked
        .metadata
        .provisioning
        .dockerfile_sha256
        .clone_from(&inputs.dockerfile_sha256);
    crate::metadata::update(&locked.paths, &locked.metadata)?;

    let warnings = image::cleanup_stale_archives(&locked.paths)?;

    let output = provisioning::provision(
        &mut locked,
        &inputs,
        preconditions,
        host,
        workspace_root,
        progress,
        warnings,
    )?;

    // 成果物をread-onlyで再確認できてからintentをclearする。clearのatomic replaceに失敗
    // した場合も、disk上のintentは残るため、次回の明示repairへ安全に渡る。
    let completed = provisioning::observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;
    if !completed.is_complete() {
        return Err(provisioning::require_repair(
            &locked.metadata,
            provisioning::ProvisioningState::Pending,
        ));
    }
    locked.metadata.initial_provisioning = None;
    locked.metadata.declared_files = Some(provisioning::initial_intent(&inputs).files);
    crate::metadata::update(&locked.paths, &locked.metadata)?;
    Ok(output)
}

fn ready_output(
    metadata: &crate::metadata::ProjectMetadata,
    observation: &provisioning::Observation,
) -> PrepareOutput {
    PrepareOutput {
        project: metadata.display_id(),
        sandbox: metadata.sandbox_name().to_string(),
        mode: metadata.provisioning.mode,
        start_ref: metadata.provisioning.start_ref.clone().unwrap_or_default(),
        sandbox_state: observation
            .sandbox_state
            .unwrap_or(crate::boundary::host::protocol::SandboxState::Running),
        worktrees: observation.worktrees.clone(),
        files: observation.files.clone(),
        already_built: true,
        warnings: Vec::new(),
    }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "generation_test.rs"]
mod generation_test;

#[cfg(test)]
#[path = "intent_test.rs"]
mod intent_test;

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
