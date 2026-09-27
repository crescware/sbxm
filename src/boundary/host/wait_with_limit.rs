use std::process::ExitStatus;
use std::time::Duration;

use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;
use crate::time::Clock;

use super::{CommandSpec, Processes, WAIT_POLL_INTERVAL, terminate_child, unwaitable};

/// 子の終了を待つ。`limit`を過ぎたら子を終わらせて報告する。
///
/// `tick`があれば、待つあいだ`every`ごとにその手続きを呼ぶ。手続きはこのthreadで走り、
/// 長くかかれば子の終了に気付くのもその分だけ遅れる。
pub(super) fn wait_with_limit<O: Processes>(
    os: &O,
    clock: &dyn Clock,
    child: &mut O::Child,
    spec: &CommandSpec,
    limit: Option<Duration>,
    tick: Option<(Duration, &mut dyn FnMut())>,
) -> Result<ExitStatus> {
    if limit.is_none() && tick.is_none() {
        // 対話processは、利用者が終えるまで待つ。
        return match os.wait_exit(child) {
            Ok(status) => Ok(status),
            Err(error) => Err(unwaitable(os, child, spec, &error)),
        };
    }
    let deadline = limit.map(|limit| (clock.now().after(limit), limit));
    let mut ticking = tick.map(|(every, action)| (clock.now().after(every), every, action));
    loop {
        match os.check_exit(child) {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if let Some((deadline, limit)) = deadline
                    && clock.now() >= deadline
                {
                    // 期限を過ぎたcommandは、報告より先に終わらせる。
                    terminate_child(os, child);
                    return fail(
                        ErrorId::ExternalCommandTimeout,
                        msg!(
                            "error-external-command-timeout",
                            program = spec.program,
                            seconds = limit.as_secs()
                        ),
                    );
                }
                if let Some((at, every, action)) = ticking.as_mut()
                    && clock.now() >= *at
                {
                    action();
                    // 手続きにかかった時間の後から、次の間隔を数える。
                    *at = clock.now().after(*every);
                    continue;
                }
                clock.sleep(WAIT_POLL_INTERVAL);
            }
            Err(error) => return Err(unwaitable(os, child, spec, &error)),
        }
    }
}
