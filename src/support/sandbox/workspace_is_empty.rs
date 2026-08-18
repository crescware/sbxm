use std::path::Path;

use crate::diagnostics::Result;
use crate::paths::PathScope;
use crate::project::SandboxName;

use super::{workspace_exists, workspace_path};

/// workspace directoryが空であることをread-onlyで確かめる。
#[allow(dead_code)]
pub fn workspace_is_empty(workspace_root: &Path, sandbox: &SandboxName) -> Result<bool> {
    let path = workspace_path(workspace_root, sandbox);
    if !workspace_exists(workspace_root, sandbox)? {
        return Ok(true);
    }
    let mut entries = std::fs::read_dir(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    let first = entries
        .next()
        .transpose()
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    Ok(first.is_none())
}
