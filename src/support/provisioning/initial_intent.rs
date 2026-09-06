use crate::metadata::{InitialProvisioningFile, InitialProvisioningIntent};

use super::ProvisioningInputs;

/// 最初のmutationの前に保存する、初回構築の固定intentを作る。
///
/// `inputs`が固定したsnapshotから作るため、intentのdigestは実際にbuild・配置へ渡す
/// byte列そのものと一致する。
pub(crate) fn initial_intent(inputs: &ProvisioningInputs) -> InitialProvisioningIntent {
    let files = inputs
        .files
        .iter()
        .map(|file| InitialProvisioningFile {
            source: file.original_source.clone(),
            destination: crate::paths::display(file.declaration.destination.as_path()),
            sha256: file.sha256.clone(),
        })
        .collect();
    InitialProvisioningIntent {
        target_dockerfile_sha256: inputs.dockerfile_sha256.clone(),
        files,
    }
}
