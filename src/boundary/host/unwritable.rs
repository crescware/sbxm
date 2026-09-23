use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use super::CommandSpec;

/// 子のstdinへ書き切れなかったことを、原因の原文とともに報告する。
pub(super) fn unwritable(spec: &CommandSpec, cause: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandInputUnwritable,
            msg!("error-external-command-input-unwritable"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::cause(cause)),
    )
}
