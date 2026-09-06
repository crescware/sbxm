use std::path::Path;

use crate::diagnostics::Result;

use super::read_source_bytes;

/// sourceを検証し、そのSHA-256を返す。
pub fn read_source(source: &Path) -> Result<String> {
    Ok(read_source_bytes(source)?.1)
}
