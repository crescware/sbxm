use crate::metadata::InitialProvisioningFile;

use super::SnapshotFile;

/// Snapshotからmetadataへ記録する宣言fileの固定入力を作る。
pub(crate) fn recorded_files(inputs: &[SnapshotFile]) -> Vec<InitialProvisioningFile> {
    inputs
        .iter()
        .map(|file| InitialProvisioningFile {
            source: file.original_source.clone(),
            destination: crate::paths::display(file.declaration.destination.as_path()),
            sha256: file.sha256.clone(),
        })
        .collect()
}
