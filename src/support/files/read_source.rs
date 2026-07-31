use std::fs;
use std::path::Path;

use crate::diagnostics::{Error, ErrorId, Result};
use crate::hash::sha256_hex;
use crate::msg;
use crate::paths;

use super::MAX_SOURCE_BYTES;

/// sourceを検証し、そのSHA-256を返す。
pub(super) fn read_source(source: &Path) -> Result<String> {
    let invalid = |detail: String| {
        Err(Error::new(
            ErrorId::DeclaredFileUnusable,
            msg!(
                "error-declared-file-unusable",
                source = paths::display(source),
                detail = detail
            ),
        ))
    };

    if !source.is_absolute() {
        return invalid("the source is not an absolute path".to_string());
    }
    let metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) => return invalid(format!("the source could not be read: {error}")),
    };
    if metadata.file_type().is_symlink() {
        return invalid("the source is a symbolic link".to_string());
    }
    if !metadata.is_file() {
        return invalid("the source is not a regular file".to_string());
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return invalid(format!(
            "the source is {} bytes, and sbxm places at most {MAX_SOURCE_BYTES}",
            metadata.len()
        ));
    }

    match fs::read(source) {
        // 内容は診断へ出さず、比較に使うdigestだけを持つ。
        Ok(contents) => Ok(sha256_hex(&contents)),
        Err(error) => invalid(format!("the source could not be read: {error}")),
    }
}
