use std::fs;
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PathScope};

/// 既存configの原文。存在しなければ`None`。
pub(super) fn read_existing(path: &Path) -> Result<Option<String>> {
    if !paths::regular_file_exists(path, PathScope::ConfigFile)? {
        return Ok(None);
    }
    fs::read_to_string(path).map(Some).map_err(|error| {
        Error::single(
            Diagnostic::new(ErrorId::ConfigUnreadable, msg!("error-config-unreadable"))
                .fact(Fact::path(&paths::display(path)))
                .fact(Fact::cause(&error.to_string())),
        )
    })
}
