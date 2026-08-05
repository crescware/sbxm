use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use super::CommandSpec;

/// 出力を読み切れなかったことを、原因の原文とともに報告する。
pub(super) fn unreadable(spec: &CommandSpec, cause: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandOutputUnreadable,
            msg!("error-external-command-output-unreadable"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::cause(cause)),
    )
}
