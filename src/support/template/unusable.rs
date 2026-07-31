use crate::diagnostics::{Error, ErrorId};
use crate::msg;

pub(super) fn unusable(name: &str, detail: &str) -> Error {
    Error::new(
        ErrorId::TemplateUnusable,
        msg!("error-template-unusable", template = name, detail = detail),
    )
}
