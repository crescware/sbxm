use std::time::Duration;

use crate::diagnostics::Msg;
use crate::msg;

pub(super) enum RetryReason {
    Timeout,
    Unavailable,
    Incomplete,
}

impl RetryReason {
    pub(super) fn message(self, attempt: u32, maximum: u32, delay: Duration) -> Msg {
        match self {
            RetryReason::Timeout => msg!(
                "progress-retry-login-timeout",
                attempt = attempt,
                maximum = maximum,
                seconds = delay.as_secs()
            ),
            RetryReason::Unavailable => msg!(
                "progress-retry-login-unavailable",
                attempt = attempt,
                maximum = maximum,
                seconds = delay.as_secs()
            ),
            RetryReason::Incomplete => msg!(
                "progress-retry-login-incomplete",
                attempt = attempt,
                maximum = maximum,
                seconds = delay.as_secs()
            ),
        }
    }
}
