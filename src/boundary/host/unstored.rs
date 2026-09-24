use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use super::CommandSpec;

/// 受け取った出力を、hostの受け手へ書けなかったことを、原因の原文とともに報告する。
///
/// 出力を読めなかったのではない。hostのdiskが埋まった場合のように、受け手の側の失敗と
/// して区別する。
pub(super) fn unstored(spec: &CommandSpec, cause: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandOutputUnstored,
            msg!("error-external-command-output-unstored"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::cause(cause)),
    )
}
