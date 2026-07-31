use std::path::Path;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::{FileDeclaration, HostFileSource, RawFile, SandboxHomeRelativePath, missing_field};

/// 宣言されたfileを読む。
pub(super) fn parse_files(raw: Vec<RawFile>, path: &Path) -> Result<Vec<FileDeclaration>> {
    let mut files = Vec::with_capacity(raw.len());
    for (index, entry) in raw.into_iter().enumerate() {
        let source_value = entry
            .source
            .ok_or_else(|| missing_field(path, "files.source"))?;
        let destination_value = entry
            .destination
            .ok_or_else(|| missing_field(path, "files.destination"))?;
        let source = HostFileSource::new(&source_value).map_err(|detail| {
            Error::new(
                ErrorId::FileDeclarationInvalidSource,
                msg!(
                    "error-file-declaration-invalid-source",
                    index = index,
                    source = source_value,
                    detail = detail
                ),
            )
        })?;
        let destination = SandboxHomeRelativePath::new(&destination_value).map_err(|detail| {
            Error::new(
                ErrorId::FileDeclarationInvalidDestination,
                msg!(
                    "error-file-declaration-invalid-destination",
                    index = index,
                    destination = destination_value,
                    detail = detail
                ),
            )
        })?;
        files.push(FileDeclaration {
            source,
            destination,
        });
    }
    Ok(files)
}
