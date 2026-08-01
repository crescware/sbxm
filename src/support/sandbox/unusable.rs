use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;

pub(super) fn unusable(name: &str, reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::SandboxUnusable, msg!("error-sandbox-unusable"))
            .fact(Fact::sandbox(name))
            .fact(Fact::reason(reason)),
    )
}
