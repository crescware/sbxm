use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;

/// 配置先として受け付けられない値を、渡された綴りと理由を添えて断る。
pub fn invalid_destination(given: &str, reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::FileDeclarationInvalidDestination,
            msg!("error-file-declaration-invalid-destination"),
        )
        .fact(Fact::destination(given))
        .fact(Fact::reason(reason)),
    )
}
