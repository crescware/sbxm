use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
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
            return Err(Error::single(
                Diagnostic::new(ErrorId::ConfigUnreadable, msg!("error-config-unreadable"))
                    .fact(Fact::path(&paths::display(&path)))
                    .fact(Fact::cause(&error.to_string())),
            ));
        }
    };

    if !metadata.is_file() {
        return Err(Error::single(
            Diagnostic::new(ErrorId::ConfigUnreadable, msg!("error-config-unreadable"))
                .fact(Fact::path(&paths::display(&path)))
                .fact(Fact::reason(msg!("cause-not-a-regular-file"))),
        ));
    }

    let mode = metadata.permissions().mode();
    if permission_too_open(mode) {
        return Err(PathScope::ConfigFile.permission_error(&path, mode, PRIVATE_FILE_MODE));
    }

    let text = fs::read_to_string(&path).map_err(|error| {
        Error::single(
            Diagnostic::new(ErrorId::ConfigUnreadable, msg!("error-config-unreadable"))
                .fact(Fact::path(&paths::display(&path)))
                .fact(Fact::cause(&error.to_string())),
        )
    })?;

    parse(&text, &path)
}
