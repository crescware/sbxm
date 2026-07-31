use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use crate::design::Remediation;

/// 端末が読めなかったことを報告する。
///
/// 中断は何も変更せず終える。それ以外を引数の不足として報告しない。
pub fn unreadable(error: &std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::Interrupted {
        return Error::Canceled;
    }
    Error::single(
        Diagnostic::new(
            ErrorId::PromptUnreadable,
            msg!("error-prompt-unreadable", detail = error),
        )
        .remediation(Remediation::text(msg!("remediation-prompt-unreadable"))),
    )
}
