use crate::metadata::InitialProvisioningFile;

use super::SnapshotFile;

/// 1件のsnapshotから、metadataへ記録する宣言fileの固定入力を作る。
pub(crate) fn recorded_file(input: &SnapshotFile) -> InitialProvisioningFile {
    InitialProvisioningFile {
        source: input.original_source.clone(),
        destination: crate::paths::display(input.declaration.destination.as_path()),
        sha256: input.sha256.clone(),
    }
}
