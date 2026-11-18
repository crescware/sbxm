use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::boundary::host::protocol::parse_wake_time;
use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::diagnostics::{Error, Result};
use crate::msg;
use crate::time::{Clock, Moment};

/// Dockerが通知を受けた時刻とOSの復帰時刻には差があるため、解除時刻は目安である。
/// 120秒はsbx v0.51.0の`sandboxlib/auth/wake_state.go`にあるhold-offの長さ。
pub(super) struct ResumeHoldoff {
    until: SystemTime,
    deadline: Moment,
}

impl ResumeHoldoff {
    /// 制限の明示後だけOSを読む。取得不能、不正、未来、既に120秒以上前の復帰は使わない。
    pub(super) fn observe(host: &dyn HostEnvironment, clock: &dyn Clock) -> Result<Option<Self>> {
        let Some(output) = capture(
            host,
            &CommandSpec::probe("sysctl", &["-n", "kern.waketime"]),
        )?
        else {
            return Ok(None);
        };
        let Some(wake) = parse_wake_time(&output) else {
            return Ok(None);
        };
        let now = clock.wall();
        let Ok(elapsed) = now.duration_since(wake) else {
            return Ok(None);
        };
        let window = Duration::from_secs(120);
        if elapsed >= window {
            return Ok(None);
        }
        Ok(wake.checked_add(window).map(|until| Self {
            until,
            deadline: clock.now().after(window - elapsed),
        }))
    }

    pub(super) fn remaining(&self, clock: &dyn Clock) -> Duration {
        self.deadline.since(clock.now())
    }

    /// macOSのdateでhostのlocal timeにする。表示のためにtimezoneの実装や設定を複製しない。
    pub(super) fn explain(&self, host: &dyn HostEnvironment, mut error: Error) -> Result<Error> {
        let epoch = self.until.duration_since(UNIX_EPOCH).unwrap_or_default();
        // 秒の途中で解除される場合は切り上げ、解除より前の時刻を案内しない。
        let seconds = epoch.as_secs() + u64::from(epoch.subsec_nanos() != 0);
        let spec = CommandSpec::probe("date", &["-r", &seconds.to_string(), "+%H:%M:%S"]);
        if let Some(output) = capture(host, &spec)?
            && valid_time(output.trim())
            && let Error::Diagnostics(diagnostics) = &mut error
            && let Some(diagnostic) = diagnostics.first_mut()
        {
            diagnostic.description = msg!(
                "notice-sbx-login-resume-holdoff-until",
                time = output.trim()
            );
        }
        Ok(error)
    }
}

fn capture(host: &dyn HostEnvironment, spec: &CommandSpec) -> Result<Option<String>> {
    match host.run(spec).and_then(CommandOutcome::require_success) {
        Ok(outcome) => Ok(Some(outcome.stdout_text())),
        Err(Error::Canceled) => Err(Error::Canceled),
        Err(_) => Ok(None),
    }
}

fn valid_time(time: &str) -> bool {
    let fields: Vec<_> = time.split(':').collect();
    fields.len() == 3
        && fields.iter().zip([24, 60, 60]).all(|(field, limit)| {
            field.len() == 2
                && field.bytes().all(|byte| byte.is_ascii_digit())
                && field.parse::<u8>().is_ok_and(|value| value < limit)
        })
}
