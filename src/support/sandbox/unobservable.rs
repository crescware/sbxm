use crate::boundary::host::CommandOutcome;
use crate::diagnostics::{Error, ErrorId};
use crate::msg;

/// 内側のcommandが答えなかった場合の診断。原値をそのまま残す。
pub fn unobservable(outcome: &CommandOutcome, subject: &str) -> Error {
    Error::single(
        crate::diagnostics::Diagnostic::new(
            ErrorId::SandboxCheckUnobservable,
            msg!(
                "error-sandbox-check-unobservable",
                subject = subject,
                exit_status = outcome.status
            ),
        )
        .external(outcome.failure()),
    )
}
