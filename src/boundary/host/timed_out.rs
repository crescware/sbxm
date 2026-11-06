use std::time::Duration;

use crate::diagnostics::{Diagnostic, Error, ErrorId, ExternalFailure};
use crate::msg;

use super::CommandSpec;

/// 終了statusのない時間切れでも、起動したcommandを残す。
pub(super) fn timed_out(spec: &CommandSpec, limit: Duration) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandTimeout,
            msg!(
                "error-external-command-timeout",
                program = spec.program,
                seconds = limit.as_secs()
            ),
        )
        .external(ExternalFailure {
            program: spec.program.clone(),
            safe_args: spec.args.clone(),
            working_dir: spec.working_dir.clone(),
            exit_status: None,
            stderr: Vec::new(),
            stderr_lossy: false,
        }),
    )
}
