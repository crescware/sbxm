use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::config::ConfigLocation;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope, permission_too_open};

use super::{Index, parse};

/// registryをread-onlyで読む。
///
/// fileが存在しないことは、登録案件0件として正常に扱う。読めたが不正なregistryを
/// 0件へ丸めない。
pub fn load(location: &ConfigLocation) -> Result<Index> {
    let path = location.registry_file();

    if paths::is_symlink(&path) {
        return Err(PathScope::ConfigFile.symlink_error(&path));
    }

    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Index::default());
        }
        Err(error) => return Err(unreadable(&path, &error.to_string())),
    };
    if !metadata.is_file() {
        return Err(unreadable(&path, "the registry path is not a regular file"));
    }
    let mode = metadata.permissions().mode();
    if permission_too_open(mode) {
        return Err(PathScope::ConfigFile.permission_error(&path, mode, PRIVATE_FILE_MODE));
    }

    let text = fs::read_to_string(&path).map_err(|error| unreadable(&path, &error.to_string()))?;
    let registry = parse(&text, &path)?;
    registry.check_invariants()?;
    Ok(registry)
}

fn unreadable(path: &Path, detail: &str) -> Error {
    Error::new(
        ErrorId::RegistryUnreadable,
        msg!(
            "error-registry-unreadable",
            path = paths::display(path),
            detail = detail
        ),
    )
}
