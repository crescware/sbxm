use std::fs;
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, ProjectPaths};

use super::{ProjectMetadata, parse};

/// metadataをread-onlyで読む。存在しなければ`None`。
pub fn load(paths: &ProjectPaths) -> Result<Option<ProjectMetadata>> {
    let path = paths.metadata_file();
    match read_optional(&path)? {
        Some(text) => Ok(Some(parse(&text, &path)?)),
        None => Ok(None),
    }
}

/// symlinkを追跡せずにmetadataを読む。
fn read_optional(path: &Path) -> Result<Option<String>> {
    let unreadable = |cause: Fact| {
        Error::single(
            Diagnostic::new(
                ErrorId::MetadataUnreadable,
                msg!("error-metadata-unreadable"),
            )
            .fact(Fact::path(&paths::display(path)))
            .fact(cause),
        )
    };
    if paths::is_symlink(path) {
        // symlinkの先は案件directory外にあり得るため、追跡せず不在として扱わない。
        return Err(unreadable(Fact::reason(msg!("cause-symbolic-link"))));
    }
    // 通常fileであることを確かめてから開く。FIFOのような特殊fileを開いて待たない。
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(unreadable(Fact::reason(msg!("cause-not-a-regular-file"))));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(unreadable(Fact::cause(&error.to_string())));
        }
    }
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(unreadable(Fact::cause(&error.to_string()))),
    }
}
