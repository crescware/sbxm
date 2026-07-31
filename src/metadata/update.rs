use crate::diagnostics::Result;
use crate::paths::{PRIVATE_FILE_MODE, ProjectPaths, atomic_replace};

use super::{ProjectMetadata, render};

/// 既存metadataをatomicに置き換える。
pub fn update(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<()> {
    atomic_replace(
        &paths.metadata_file(),
        &render(metadata)?,
        PRIVATE_FILE_MODE,
    )
}
