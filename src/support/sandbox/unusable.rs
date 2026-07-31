use crate::diagnostics::{Error, ErrorId};
use crate::msg;

pub(super) fn unusable(name: &str, detail: &str) -> Error {
    Error::new(
        ErrorId::SandboxUnusable,
        msg!("error-sandbox-unusable", sandbox = name, detail = detail),
    )
}
