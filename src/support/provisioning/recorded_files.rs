use crate::metadata::InitialProvisioningFile;

use super::{SnapshotFile, recorded_file};

/// Snapshotからmetadataへ記録する宣言fileの固定入力を作る。
pub(crate) fn recorded_files(inputs: &[SnapshotFile]) -> Vec<InitialProvisioningFile> {
    inputs.iter().map(recorded_file).collect()
}
