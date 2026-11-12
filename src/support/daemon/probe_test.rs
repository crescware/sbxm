use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use crate::boundary::host::{CommandOutcome, CommandSpec, TimeoutClass};
use crate::design::Fact;
use crate::diagnostics::{DiagnosticKind, Error, ErrorId, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::testing::command::{outcome, outcome_with_stderr};
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;
use crate::testing::scripted_clock::ScriptedClock;
use crate::time::{Clock, Moment};

use super::list_retrying;

const UNAVAILABLE: &str =
    "503 Service Unavailable: docker login service unavailable: temporary network failure";
const RESUME_HOLDOFF: &str = "503 Service Unavailable: docker login service unavailable: within the post-resume hold-off window docker session could not be refreshed due to a network issue";

#[derive(Clone, Copy)]
enum Reply {
    Listing(&'static str),
    Failed(&'static str),
    Timeout,
    Canceled,
    Missing,
}

struct Host<'a> {
    clock: &'a ScriptedClock,
    replies: RefCell<VecDeque<Reply>>,
    calls: RefCell<Vec<(Moment, CommandSpec)>>,
}

impl<'a> Host<'a> {
    fn new(clock: &'a ScriptedClock, replies: &[Reply]) -> Host<'a> {
        Host {
            clock,
            replies: RefCell::new(replies.iter().copied().collect()),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl AnsweredHost for Host<'_> {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls
            .borrow_mut()
            .push((self.clock.now(), spec.clone()));
        let reply = self
            .replies
            .borrow_mut()
            .pop_front()
            .unwrap_or(Reply::Listing("[]"));
        match reply {
            Reply::Listing(output) => Ok(outcome(spec, 0, output)),
            Reply::Failed(stderr) => Ok(outcome_with_stderr(spec, 1, "", stderr)),
            Reply::Timeout => {
                self.clock.advance(Duration::from_secs(10));
                Err(Error::new(
                    ErrorId::ExternalCommandTimeout,
                    msg!(
                        "error-external-command-timeout",
                        program = "sbx",
                        seconds = 10
                    ),
                ))
            }
            Reply::Canceled => Err(Error::Canceled),
            Reply::Missing => Err(Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = "sbx"),
            )),
        }
    }
}

fn probe(
    host: &Host<'_>,
    progress: &mut RecordedOutput,
) -> Result<Vec<crate::boundary::host::protocol::SandboxEntry>> {
    list_retrying(host, TimeoutClass::Probe, host.clock, progress, &|| false)
}

#[test]
fn resume_holdoff_stops_after_one_call_without_waiting_for_a_later_success() -> Checked {
    let clock = ScriptedClock::default();
    let host = Host::new(
        &clock,
        &[Reply::Failed(RESUME_HOLDOFF), Reply::Listing("[]")],
    );
    let mut progress = RecordedOutput::new();
    let error = probe(&host, &mut progress).refused_because("the host just woke up")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::SbxLoginResumeHoldoff);
    assert_eq!(diagnostic.kind, DiagnosticKind::Notice);
    assert_eq!(
        diagnostic.external.as_ref().required()?.stderr,
        RESUME_HOLDOFF.as_bytes()
    );
    assert_eq!(host.calls.borrow().len(), 1);
    assert_eq!(host.replies.borrow().len(), 1);
    assert!(clock.slept.borrow().is_empty());
    assert!(progress.steps.is_empty());
    Ok(())
}

#[test]
fn resume_holdoff_also_stops_when_it_follows_another_transient_failure() -> Checked {
    for first in [
        Reply::Timeout,
        Reply::Failed(UNAVAILABLE),
        Reply::Listing("{"),
    ] {
        let clock = ScriptedClock::default();
        let host = Host::new(
            &clock,
            &[first, Reply::Failed(RESUME_HOLDOFF), Reply::Listing("[]")],
        );
        let mut progress = RecordedOutput::new();
        let error = probe(&host, &mut progress).refused_because("recovery requires time")?;
        assert_eq!(error.first_id(), Some(ErrorId::SbxLoginResumeHoldoff));
        assert_eq!(host.calls.borrow().len(), 2);
        assert_eq!(host.replies.borrow().len(), 1);
        assert_eq!(progress.steps.len(), 1);
        let expected = if matches!(first, Reply::Timeout) {
            11
        } else {
            1
        };
        assert_eq!(
            clock.now(),
            Moment::since_origin(Duration::from_secs(expected))
        );
    }
    Ok(())
}

#[test]
fn transient_failures_and_truncated_json_share_three_attempts_with_exponential_backoff() -> Checked
{
    for replies in [
        [
            Reply::Failed(UNAVAILABLE),
            Reply::Failed(UNAVAILABLE),
            Reply::Listing("[]"),
        ],
        [
            Reply::Failed(UNAVAILABLE),
            Reply::Listing("{"),
            Reply::Listing("[]"),
        ],
        [
            Reply::Listing("{"),
            Reply::Failed(UNAVAILABLE),
            Reply::Listing("[]"),
        ],
    ] {
        let clock = ScriptedClock::default();
        let host = Host::new(&clock, &replies);
        let mut progress = RecordedOutput::new();
        probe(&host, &mut progress).required()?;
        let calls = host.calls.borrow();
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls
                .iter()
                .map(|(at, _)| at.since(Moment::since_origin(Duration::ZERO)))
                .collect::<Vec<_>>(),
            [
                Duration::ZERO,
                Duration::from_secs(1),
                Duration::from_secs(3)
            ]
        );
        assert!(
            calls
                .iter()
                .all(|(_, spec)| spec.timeout.duration() == Some(Duration::from_secs(10)))
        );
        assert_eq!(progress.steps.len(), 2);
        assert!(progress.steps[0].args.contains(&("attempt", "2".into())));
        assert!(progress.steps[0].args.contains(&("seconds", "1".into())));
        assert!(progress.steps[1].args.contains(&("attempt", "3".into())));
        assert!(progress.steps[1].args.contains(&("seconds", "2".into())));
    }
    Ok(())
}

#[test]
fn three_timeouts_take_at_most_33_seconds_and_report_the_attempts() -> Checked {
    let clock = ScriptedClock::default();
    let host = Host::new(&clock, &[Reply::Timeout; 3]);
    let mut progress = RecordedOutput::new();
    let error = probe(&host, &mut progress).refused_because("all three attempts timed out")?;
    assert_eq!(host.calls.borrow().len(), 3);
    assert_eq!(clock.now(), Moment::since_origin(Duration::from_secs(33)));
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert_eq!(
        progress
            .steps
            .iter()
            .map(|message| message.id)
            .collect::<Vec<_>>(),
        ["progress-retry-login-timeout"; 2]
    );
    assert!(error.diagnostics()[0].facts.iter().any(|fact| matches!(fact, Fact::Translated { value, .. } if value.id == "sbx-probe-attempts" && value.args.contains(&("attempts", "3".into())))));
    Ok(())
}

#[test]
fn a_timeout_can_recover_and_a_truncated_listing_cannot_start_a_second_retry_budget() -> Checked {
    for replies in [
        [Reply::Timeout, Reply::Listing("{"), Reply::Listing("[]")],
        [Reply::Listing("{"), Reply::Timeout, Reply::Listing("{")],
    ] {
        let clock = ScriptedClock::default();
        let host = Host::new(&clock, &replies);
        let result = probe(&host, &mut RecordedOutput::new());
        assert_eq!(host.calls.borrow().len(), 3);
        match replies[2] {
            Reply::Listing("[]") => {
                result.required()?;
            }
            _ => {
                assert_eq!(
                    result
                        .refused_because("the shared budget is exhausted")?
                        .first_id(),
                    Some(ErrorId::ExternalOutputUnparseable)
                );
            }
        }
    }
    Ok(())
}

#[test]
fn permanent_failures_and_cancellation_do_not_retry() -> Checked {
    for reply in [
        Reply::Failed("401 Unauthorized: user is not authenticated to Docker"),
        Reply::Failed("503 Service Unavailable: another service"),
        Reply::Failed("permission denied"),
        Reply::Missing,
        Reply::Canceled,
        Reply::Listing("null"),
        Reply::Listing("not JSON"),
    ] {
        let clock = ScriptedClock::default();
        let host = Host::new(&clock, &[reply]);
        let mut progress = RecordedOutput::new();
        probe(&host, &mut progress).refused_because("this failure is not temporary")?;
        assert_eq!(host.calls.borrow().len(), 1);
        assert!(clock.slept.borrow().is_empty());
        assert!(progress.steps.is_empty());
    }
    Ok(())
}

#[test]
fn recovery_on_the_second_attempt_does_not_wait_again() -> Checked {
    let clock = ScriptedClock::default();
    let host = Host::new(&clock, &[Reply::Failed(UNAVAILABLE), Reply::Listing("[]")]);
    probe(&host, &mut RecordedOutput::new()).required()?;
    assert_eq!(host.calls.borrow().len(), 2);
    assert_eq!(clock.now(), Moment::since_origin(Duration::from_secs(1)));
    Ok(())
}

#[test]
fn a_later_authentication_failure_stops_with_the_latest_stderr() -> Checked {
    let clock = ScriptedClock::default();
    let host = Host::new(
        &clock,
        &[
            Reply::Failed(UNAVAILABLE),
            Reply::Failed("user is not authenticated to Docker"),
        ],
    );
    let error =
        probe(&host, &mut RecordedOutput::new()).refused_because("signing in is necessary")?;
    assert_eq!(error.first_id(), Some(ErrorId::SbxLoginMissing));
    assert_eq!(host.calls.borrow().len(), 2);
    assert_eq!(
        error.diagnostics()[0].external.as_ref().required()?.stderr,
        b"user is not authenticated to Docker"
    );
    Ok(())
}

#[test]
fn an_unavailable_service_stops_after_three_attempts_and_later_cancellation_stays_canceled()
-> Checked {
    for last in [Reply::Failed(UNAVAILABLE), Reply::Canceled] {
        let clock = ScriptedClock::default();
        let host = Host::new(
            &clock,
            &[Reply::Failed(UNAVAILABLE), Reply::Listing("{"), last],
        );
        let error =
            probe(&host, &mut RecordedOutput::new()).refused_because("the third attempt failed")?;
        assert_eq!(host.calls.borrow().len(), 3);
        if let Reply::Canceled = last {
            assert_eq!(error, Error::Canceled);
        } else {
            assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
            assert_eq!(
                error.diagnostics()[0].external.as_ref().required()?.stderr,
                UNAVAILABLE.as_bytes()
            );
        }
    }
    Ok(())
}

#[test]
fn cancellation_before_an_attempt_or_during_backoff_starts_no_more_commands() -> Checked {
    for cancel_after in [
        Duration::ZERO,
        Duration::from_millis(100),
        Duration::from_secs(1),
    ] {
        let clock = ScriptedClock::default();
        let host = Host::new(&clock, &[Reply::Failed(UNAVAILABLE)]);
        let error = list_retrying(
            &host,
            TimeoutClass::Probe,
            &clock,
            &mut RecordedOutput::new(),
            &|| clock.now() >= Moment::since_origin(cancel_after),
        )
        .refused_because("the user canceled")?;
        assert_eq!(error, Error::Canceled);
        assert_eq!(
            host.calls.borrow().len(),
            usize::from(!cancel_after.is_zero())
        );
    }
    Ok(())
}

#[test]
fn lifecycle_observation_does_not_retry_transient_service_errors() -> Checked {
    for reply in [Reply::Timeout, Reply::Failed(UNAVAILABLE)] {
        let clock = ScriptedClock::default();
        let host = Host::new(&clock, &[reply]);
        list_retrying(
            &host,
            TimeoutClass::SandboxLifecycle,
            &clock,
            &mut RecordedOutput::new(),
            &|| false,
        )
        .refused_because("only the preflight probe retries these failures")?;
        assert_eq!(host.calls.borrow().len(), 1);
        assert!(clock.slept.borrow().is_empty());
    }
    Ok(())
}

#[test]
fn retry_messages_render_the_reason_and_delay_in_both_languages() -> Checked {
    for replies in [
        [
            Reply::Timeout,
            Reply::Failed(UNAVAILABLE),
            Reply::Listing("[]"),
        ],
        [
            Reply::Listing("{"),
            Reply::Listing("[]"),
            Reply::Listing("[]"),
        ],
    ] {
        let clock = ScriptedClock::default();
        let host = Host::new(&clock, &replies);
        let mut progress = RecordedOutput::new();
        probe(&host, &mut progress).required()?;
        for locale in [Locale::En, Locale::Ja] {
            for message in &progress.steps {
                let text = Catalog::new(locale).format(message).required()?;
                assert!(text.contains('2') || text.contains('3'), "{text}");
                assert!(!text.contains("external-command"), "{text}");
            }
        }
    }
    Ok(())
}
