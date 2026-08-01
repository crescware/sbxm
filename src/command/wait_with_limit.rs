use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;

use super::{CommandSpec, WAIT_POLL_INTERVAL};

pub(super) fn wait_with_limit(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
) -> Result<ExitStatus> {
    let Some(limit) = limit else {
        // 対話processは、利用者が終えるまで待つ。
        return child.wait().map_err(|error| spawn_failed(spec, &error));
    };
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // timeout時はchildを終了する。
                    let _ = child.kill();
                    let _ = child.wait();
                    return fail(
                        ErrorId::ExternalCommandTimeout,
                        msg!(
                            "error-external-command-timeout",
                            program = spec.program,
                            seconds = limit.as_secs()
                        ),
                    );
                }
                std::thread::sleep(WAIT_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(spawn_failed(spec, &error));
            }
        }
    }
}

/// 子processを待てなかったことを報告する。原因はOSが書いた原文である。
fn spawn_failed(spec: &CommandSpec, error: &std::io::Error) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandSpawnFailed,
            msg!("error-external-command-spawn-failed"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::cause(&error.to_string())),
    )
}
