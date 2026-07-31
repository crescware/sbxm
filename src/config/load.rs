use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::diagnostics::{Error, ErrorId, Result, fail};
use crate::msg;
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope, permission_too_open};

use super::{ConfigLocation, ConfigState, parse};

/// configをread-onlyで読み、存在と妥当性を判定する。
///
/// 構文不正、未知version、必須値欠落、permission過剰、symlink、relative base pathは
/// pathと原因を示すerrorとし、自動修復しない。
pub fn load(location: &ConfigLocation) -> Result<ConfigState> {
    let path = location.config_file();

    if paths::is_symlink(&path) {
        return Err(PathScope::ConfigFile.symlink_error(&path));
    }

    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConfigState::Missing);
        }
        Err(error) => {
            return fail(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = paths::display(&path),
                    detail = error
                ),
            );
        }
    };

    if !metadata.is_file() {
        return fail(
            ErrorId::ConfigUnreadable,
            msg!(
                "error-config-unreadable",
                path = paths::display(&path),
                detail = "the configuration path is not a regular file"
            ),
        );
    }

    let mode = metadata.permissions().mode();
    if permission_too_open(mode) {
        return Err(PathScope::ConfigFile.permission_error(&path, mode, PRIVATE_FILE_MODE));
    }

    let text = fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorId::ConfigUnreadable,
            msg!(
                "error-config-unreadable",
                path = paths::display(&path),
                detail = error
            ),
        )
    })?;

    parse(&text, &path)
}
