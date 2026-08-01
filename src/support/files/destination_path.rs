use std::path::{Component, Path};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::paths;

/// `agent` homeからの相対pathとして安全であることを確認する。
pub(super) fn destination_path(destination: &Path) -> Result<String> {
    let invalid = |reason: Msg| {
        Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::destination(&paths::display(destination)))
            .fact(Fact::reason(reason)),
        ))
    };

    if destination.is_absolute() {
        return invalid(msg!("cause-unexpectedly-absolute"));
    }
    let mut parts = Vec::new();
    for component in destination.components() {
        match component {
            Component::Normal(part) => match part.to_str() {
                Some(part) => parts.push(part.to_string()),
                None => return invalid(msg!("cause-not-valid-utf8")),
            },
            Component::CurDir => {}
            _ => return invalid(msg!("cause-leaves-agent-home")),
        }
    }
    if parts.is_empty() {
        return invalid(msg!("cause-value-empty"));
    }
    Ok(parts.join("/"))
}
