use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;

pub(super) fn unusable(name: &str, reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::TemplateUnusable, msg!("error-template-unusable"))
            .fact(Fact::template(name))
            .fact(Fact::reason(reason)),
    )
}
