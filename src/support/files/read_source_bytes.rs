use std::fs;
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::hash::sha256_hex;
use crate::msg;
use crate::paths;

use super::MAX_SOURCE_BYTES;

/// sourceを検証し、その内容とSHA-256を返す。
pub fn read_source_bytes(source: &Path) -> Result<(Vec<u8>, String)> {
    let invalid = |reason: Msg| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::source(&paths::display(source)))
            .fact(Fact::reason(reason)),
        ))
    };

    // OSが書いた原文は言い換えず、そのまま原因として示す。対象は`Source:`の行が示す。
    let unreadable = |error: &std::io::Error| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::source(&paths::display(source)))
            .fact(Fact::cause(&error.to_string())),
        ))
    };

    if !source.is_absolute() {
        return invalid(msg!("cause-not-absolute"));
    }
    let metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) => return unreadable(&error),
    };
    if metadata.file_type().is_symlink() {
        return invalid(msg!("cause-symbolic-link"));
    }
    if !metadata.is_file() {
        return invalid(msg!("cause-not-a-regular-file"));
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return invalid(msg!(
            "cause-larger-than",
            observed = metadata.len(),
            maximum = MAX_SOURCE_BYTES
        ));
    }

    match fs::read(source) {
        Ok(contents) => {
            let digest = sha256_hex(&contents);
            Ok((contents, digest))
        }
        Err(error) => unreadable(&error),
    }
}
