use crate::diagnostics::Result;
use crate::paths::{PRIVATE_FILE_MODE, ProjectPaths, atomic_create};

use super::{ProjectMetadata, render};

/// metadataを新規作成する。既存fileは上書きしない。
pub fn create(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<()> {
    atomic_create(
        &paths.metadata_file(),
        &render(metadata)?,
        PRIVATE_FILE_MODE,
    )
}
