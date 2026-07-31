use std::path::{Component, Path};

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::paths;

/// `agent` homeからの相対pathとして安全であることを確認する。
pub(super) fn destination_path(destination: &Path) -> Result<String> {
    let invalid = |detail: &str| {
        Err(Error::new(
            ErrorId::DeclaredFileUnusable,
            msg!(
                "error-declared-file-unusable",
                source = paths::display(destination),
                detail = detail
            ),
        ))
    };

    if destination.is_absolute() {
        return invalid("the destination is an absolute path");
    }
    let mut parts = Vec::new();
    for component in destination.components() {
        match component {
            Component::Normal(part) => match part.to_str() {
                Some(part) => parts.push(part.to_string()),
                None => return invalid("the destination is not valid UTF-8"),
            },
            Component::CurDir => {}
            _ => return invalid("the destination leaves the agent home directory"),
        }
    }
    if parts.is_empty() {
        return invalid("the destination is empty");
    }
    Ok(parts.join("/"))
}
