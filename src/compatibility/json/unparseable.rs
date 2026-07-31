use crate::diagnostics::{Error, ErrorId};
use crate::msg;

pub fn unparseable(program: &str, detail: &str) -> Error {
    Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = program,
            detail = detail
        ),
    )
}
