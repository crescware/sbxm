use crate::boundary::host::protocol::SandboxState;
use crate::metadata::ProjectMetadata;

use super::{Observation, ProvisioningOutput};

/// 既に目標構成が揃っている案件の結果。
///
/// この実行が何も変更していないことを、結果自身が持つ。
pub(crate) fn ready_output(
    metadata: &ProjectMetadata,
    observation: &Observation,
) -> ProvisioningOutput {
    ProvisioningOutput {
        project: metadata.display_id(),
        sandbox: metadata.sandbox_name().to_string(),
        mode: metadata.provisioning.mode,
        start_ref: metadata.provisioning.start_ref.clone().unwrap_or_default(),
        // 完成を観測できた案件のSandboxはrunningである。表示のためだけに起動しない。
        sandbox_state: observation.sandbox_state.unwrap_or(SandboxState::Running),
        worktrees: observation.worktrees.clone(),
        files: observation.files.clone(),
        already_built: true,
        warnings: Vec::new(),
    }
}
