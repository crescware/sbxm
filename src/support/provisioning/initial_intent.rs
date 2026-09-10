use crate::metadata::InitialProvisioningIntent;

use super::{ProvisioningInputs, recorded_files};

/// 最初のmutationの前に保存する、初回構築の固定intentを作る。
///
/// `inputs`が固定したsnapshotから作るため、intentのdigestは実際にbuild・配置へ渡す
/// byte列そのものと一致する。
pub(crate) fn initial_intent(inputs: &ProvisioningInputs) -> InitialProvisioningIntent {
    let files = recorded_files(&inputs.files);
    InitialProvisioningIntent {
        target_dockerfile_sha256: inputs.dockerfile_sha256.clone(),
        files,
    }
}
