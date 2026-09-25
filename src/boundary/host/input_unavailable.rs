use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::paths;

use super::CommandSpec;

/// 子のstdinへつなぐfileを開けなかったことを、原因の原文とともに報告する。
///
/// 書き込みの途中で止まったこととは別の失敗である。子はまだ起動していない。
pub(super) fn input_unavailable(spec: &CommandSpec, path: &Path, cause: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandInputUnavailable,
            msg!("error-external-command-input-unavailable"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::path(&paths::display(path)))
        .fact(Fact::cause(cause)),
    )
}
