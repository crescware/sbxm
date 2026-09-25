use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use super::{CommandSpec, WAIT_POLL_INTERVAL, terminate_child, unwaitable};

/// 子の終了を待つ。`limit`を過ぎたら子を終わらせて報告する。
///
/// `tick`があれば、待つあいだ`every`ごとにその手続きを呼ぶ。手続きはこのthreadで走り、
/// 長くかかれば子の終了に気付くのもその分だけ遅れる。
pub(super) fn wait_with_limit(
    child: &mut Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    mut tick: Option<(Duration, &mut dyn FnMut())>,
) -> Result<ExitStatus> {
    if limit.is_none() && tick.is_none() {
        // 対話processは、利用者が終えるまで待つ。
        return match child.wait() {
            Ok(status) => Ok(status),
            Err(error) => Err(unwaitable(child, spec, &error)),
        };
    }
    let deadline = limit.map(|limit| Instant::now() + limit);
    let mut next = tick.as_ref().map(|(every, _)| Instant::now() + *every);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if let (Some(deadline), Some(limit)) = (deadline, limit)
                    && Instant::now() >= deadline
                {
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
                if let (Some(at), Some((every, action))) = (next, tick.as_mut())
                    && Instant::now() >= at
                {
                    action();
                    next = Some(Instant::now() + *every);
                    continue;
                }
                std::thread::sleep(WAIT_POLL_INTERVAL);
            }
            Err(error) => {
                return Err(unwaitable(child, spec, &error));
            }
        }
    }
}
