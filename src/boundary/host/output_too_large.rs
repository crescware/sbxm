use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use super::CommandSpec;

/// stdoutが受け取れる大きさを超えたことを報告する。
pub(super) fn output_too_large(spec: &CommandSpec, limit: u64) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandOutputTooLarge,
            msg!("error-external-command-output-too-large", limit = limit),
        )
        .fact(Fact::command(&spec.program)),
    )
}
