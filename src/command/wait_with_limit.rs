use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;

use super::{CommandSpec, WAIT_POLL_INTERVAL, terminate_child};

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
                    terminate_child(child, spec);
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

/// 待てなくなった子processを終わらせ、待てなかったことを報告する。
///
/// 終わりを確かめられない相手をそのままにすると、出力を読むthreadはEOFに達しない。
/// 報告より先に、こちらから終わらせる。原因はOSが書いた原文である。
fn unwaitable(child: &mut Child, spec: &CommandSpec, error: &std::io::Error) -> Error {
    terminate_child(child, spec);
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandSpawnFailed,
            msg!("error-external-command-spawn-failed"),
        )
        .fact(Fact::command(&spec.program))
        .fact(Fact::cause(&error.to_string())),
    )
}
