use std::time::{Duration, UNIX_EPOCH};

use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::testing::global_status::FakeHost;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::scripted_clock::ScriptedClock;
use crate::time::Clock;

use super::ResumeHoldoff;

const WAKE: &str = "sysctl -n kern.waketime";

fn wake_output(clock: &dyn Clock, age: Duration) -> Checked<String> {
    let epoch = (clock.wall() - age).duration_since(UNIX_EPOCH).required()?;
    Ok(format!(
        "{{ sec = {}, usec = {} }}",
        epoch.as_secs(),
        epoch.subsec_micros()
    ))
}

fn notice() -> Error {
    Error::single(Diagnostic::notice(
        ErrorId::SbxLoginResumeHoldoff,
        msg!("notice-sbx-login-resume-holdoff"),
    ))
}

struct CanceledHelper {
    command: &'static str,
    wake: String,
}

impl AnsweredHost for CanceledHelper {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if spec.program == self.command {
            Err(Error::Canceled)
        } else {
            Ok(crate::testing::command::outcome(spec, 0, &self.wake))
        }
    }
}

#[test]
fn unobservable_and_inconsistent_wake_times_do_not_guess_a_deadline() -> Checked {
    let clock = ScriptedClock::default();
    for host in [
        FakeHost::new(),
        FakeHost::new().failing(WAKE, "permission denied", 1),
        FakeHost::new().timing_out(WAKE),
        FakeHost::new().responding(WAKE, "not a timeval"),
        FakeHost::new().responding(WAKE, "{ sec = 0, usec = 0 }"),
        FakeHost::new().responding(WAKE, &wake_output(&clock, Duration::from_secs(120))?),
        FakeHost::new().responding(WAKE, &wake_output(&clock, Duration::from_secs(121))?),
        FakeHost::new().responding(WAKE, "{ sec = 1790158502, usec = 0 }"),
    ] {
        assert!(ResumeHoldoff::observe(&host, &clock).required()?.is_none());
        assert!(clock.slept.borrow().is_empty());
    }
    Ok(())
}

#[test]
fn optional_observations_preserve_cancellation() -> Checked {
    let clock = ScriptedClock::default();
    let host = CanceledHelper {
        command: "sysctl",
        wake: String::new(),
    };
    assert_eq!(
        ResumeHoldoff::observe(&host, &clock).err().required()?,
        Error::Canceled
    );
    let host = CanceledHelper {
        command: "date",
        wake: wake_output(&clock, Duration::from_secs(45))?,
    };
    let holdoff = ResumeHoldoff::observe(&host, &clock)
        .required()?
        .required()?;
    assert_eq!(
        holdoff
            .explain(&host, notice())
            .refused_because("the observation was canceled")?,
        Error::Canceled
    );
    Ok(())
}

#[test]
fn the_deadline_counts_down_without_restarting_from_the_next_observation() -> Checked {
    let clock = ScriptedClock::default();
    let host = FakeHost::new().responding(WAKE, &wake_output(&clock, Duration::from_secs(102))?);
    let holdoff = ResumeHoldoff::observe(&host, &clock)
        .required()?
        .required()?;
    assert_eq!(holdoff.remaining(&clock), Duration::from_secs(18));
    clock.advance(Duration::from_secs(9));
    assert_eq!(holdoff.remaining(&clock), Duration::from_secs(9));
    clock.advance(Duration::from_secs(20));
    assert_eq!(holdoff.remaining(&clock), Duration::ZERO);
    Ok(())
}

#[test]
fn retry_time_uses_the_host_timezone_and_rounds_up_fractional_seconds() -> Checked {
    let clock = ScriptedClock::default();
    let wake = wake_output(&clock, Duration::from_micros(44_999_999))?;
    let until = (clock.wall() + Duration::from_secs(76))
        .duration_since(UNIX_EPOCH)
        .required()?
        .as_secs();
    let host = FakeHost::new()
        .responding(WAKE, &wake)
        .responding(&format!("date -r {until} +%H:%M:%S"), "01:16:17\n");
    let holdoff = ResumeHoldoff::observe(&host, &clock)
        .required()?
        .required()?;
    let error = holdoff.explain(&host, notice()).required()?;
    for locale in [Locale::En, Locale::Ja] {
        let text = Catalog::new(locale)
            .format(&error.diagnostics()[0].description)
            .required()?;
        assert!(text.contains("01:16:17"), "{text}");
    }
    Ok(())
}

#[test]
fn unavailable_or_malformed_local_time_keeps_the_simple_notice() -> Checked {
    let clock = ScriptedClock::default();
    let wake = wake_output(&clock, Duration::from_secs(45))?;
    let until = (clock.wall() + Duration::from_secs(75))
        .duration_since(UNIX_EPOCH)
        .required()?
        .as_secs();
    let date = format!("date -r {until} +%H:%M:%S");
    for host in [
        FakeHost::new(),
        FakeHost::new().failing(&date, "unavailable", 1),
        FakeHost::new().timing_out(&date),
        FakeHost::new().responding(&date, ""),
        FakeHost::new().responding(&date, "01:02"),
        FakeHost::new().responding(&date, "1:02:03"),
        FakeHost::new().responding(&date, "01:02:03:04"),
        FakeHost::new().responding(&date, "aa:02:03"),
        FakeHost::new().responding(&date, "24:02:03"),
        FakeHost::new().responding(&date, "01:60:03"),
        FakeHost::new().responding(&date, "01:02:60"),
        FakeHost::new().responding(&date, "01:02:03\ninjected output"),
    ] {
        let host = host.responding(WAKE, &wake);
        let holdoff = ResumeHoldoff::observe(&host, &clock)
            .required()?
            .required()?;
        assert_eq!(holdoff.explain(&host, notice()).required()?, notice());
    }
    Ok(())
}
