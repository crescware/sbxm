use std::time::Duration;

use crate::boundary::host::protocol::{
    SandboxEntry, is_listing_cut_short, is_login_resume_holdoff, is_login_unavailable,
    parse_sandbox_list,
};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::time::Clock;

use super::RetryReason;

const MAX_ATTEMPTS: u32 = 3;
const WARMUP_RETRY_DELAY: Duration = Duration::from_millis(200);
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 案件選択前のprobeは10秒ずつ最大3回、1秒・2秒の待機で再試行する。
/// 復帰直後の更新抑止は、再試行せず短い案内で停止する。
/// 一覧のJSON不全も同じ回数に含める。通常のlifecycle観測は従来どおりJSON不全だけを再試行する。
pub(super) fn list_retrying(
    host: &dyn HostEnvironment,
    timeout: TimeoutClass,
    clock: &dyn Clock,
    progress: &mut dyn ProgressSink,
    interrupted: &dyn Fn() -> bool,
) -> Result<Vec<SandboxEntry>> {
    let spec = CommandSpec::capture("sbx", &["ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(timeout);

    let probe = timeout == TimeoutClass::Probe;
    let mut attempts = 1;
    loop {
        if interrupted() {
            return Err(Error::Canceled);
        }
        let (result, retry) = attempt(host, &spec);
        match result {
            Ok(entries) => return Ok(entries),
            Err(error) => {
                let retry =
                    retry.filter(|reason| probe || matches!(reason, RetryReason::Incomplete));
                if let Some(reason) = retry
                    && attempts < MAX_ATTEMPTS
                {
                    let delay = if probe {
                        Duration::from_secs(1 << (attempts - 1))
                    } else {
                        WARMUP_RETRY_DELAY
                    };
                    attempts += 1;
                    if probe {
                        progress.step(reason.message(attempts, MAX_ATTEMPTS, delay));
                        pause(clock, delay, interrupted)?;
                    } else {
                        clock.sleep(delay);
                    }
                } else {
                    return Err(with_attempts(error, probe, attempts));
                }
            }
        }
    }
}

fn attempt(
    host: &dyn HostEnvironment,
    spec: &CommandSpec,
) -> (Result<Vec<SandboxEntry>>, Option<RetryReason>) {
    match host
        .run(spec)
        .and_then(crate::boundary::host::CommandOutcome::require_success)
    {
        Ok(outcome) => {
            let output = outcome.stdout_text();
            let result = parse_sandbox_list(&output);
            let retry = result
                .as_ref()
                .err()
                .filter(|error| {
                    error.contains_id(ErrorId::ExternalOutputUnparseable)
                        && is_listing_cut_short(&output)
                })
                .map(|_| RetryReason::Incomplete);
            (result, retry)
        }
        Err(error) => {
            if error.contains_id(ErrorId::ExternalCommandFailed)
                && let Some(external) = error
                    .diagnostics()
                    .iter()
                    .filter_map(|diagnostic| diagnostic.external.as_ref())
                    .find(|external| is_login_resume_holdoff(&external.stderr))
            {
                return (
                    Err(Error::single(
                        Diagnostic::notice(
                            ErrorId::SbxLoginResumeHoldoff,
                            msg!("notice-sbx-login-resume-holdoff"),
                        )
                        .external(external.clone()),
                    )),
                    None,
                );
            }
            let retry = if error.contains_id(ErrorId::ExternalCommandTimeout) {
                Some(RetryReason::Timeout)
            } else if error.contains_id(ErrorId::ExternalCommandFailed)
                && error.diagnostics().iter().any(|diagnostic| {
                    diagnostic
                        .external
                        .as_ref()
                        .is_some_and(|external| is_login_unavailable(&external.stderr))
                })
            {
                Some(RetryReason::Unavailable)
            } else {
                None
            };
            (Err(error), retry)
        }
    }
}

fn pause(clock: &dyn Clock, delay: Duration, interrupted: &dyn Fn() -> bool) -> Result<()> {
    let deadline = clock.now().after(delay);
    loop {
        if interrupted() {
            return Err(Error::Canceled);
        }
        let remaining = deadline.since(clock.now());
        if remaining.is_zero() {
            return Ok(());
        }
        clock.sleep(remaining.min(CANCEL_POLL_INTERVAL));
    }
}

fn with_attempts(mut error: Error, probe: bool, attempts: u32) -> Error {
    if probe
        && attempts > 1
        && let Error::Diagnostics(diagnostics) = &mut error
        && let Some(diagnostic) = diagnostics.first_mut()
    {
        diagnostic.facts.push(Fact::Translated {
            label: msg!("diagnostic-attempts-label"),
            value: msg!("sbx-probe-attempts", attempts = attempts, seconds = 10),
        });
    }
    error
}
