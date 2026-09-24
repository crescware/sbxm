use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use super::{CommandSpec, WAIT_POLL_INTERVAL, terminate_child, unwaitable};

pub(super) fn wait_with_limit(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
) -> Result<ExitStatus> {
    let Some(limit) = limit else {
        // 対話processは、利用者が終えるまで待つ。
        return match child.wait() {
            Ok(status) => Ok(status),
            Err(error) => Err(unwaitable(child, spec, &error)),
        };
    };
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // 期限を過ぎたcommandは、報告より先に終わらせる。
                    terminate_child(child);
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
                return Err(unwaitable(child, spec, &error));
            }
        }
    }
}
