use std::io::Error as IoError;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use super::CommandSpec;

/// commandを1回も走らせられなかったこと、または走らせた子を追えなくなったことの報告。
///
/// 起動できなかった子と、途中で待てなくなった子は、利用者から見ればどちらも「この実行は
/// 成立しなかった」である。どちらも手掛かりはOSが書いた原文しかないため、同じ診断を返す。
/// 4か所で同じ診断を組み立てていたものを、ここ1か所に置く。
pub(super) fn spawn_failure(spec: &CommandSpec, error: &IoError) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandSpawnFailed,
            msg!("error-external-command-spawn-failed"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::cause(&error.to_string())),
    )
}
