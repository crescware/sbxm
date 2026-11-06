use crate::boundary::host::{CommandOutcome, CommandSpec, EnvPolicy, OutputPolicy, TimeoutClass};
use crate::design::{RenderingPolicy, Ui};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::i18n::Locale;
use crate::testing::command::{ReadStep, ScriptedOs, ScriptedPipe};
use crate::testing::global_status::FakeHost;
use crate::testing::host::{AnsweredHost, FakeSbx};
use crate::testing::outcome::{Checked, Refused, Required};

#[test]
fn login_is_checked_with_a_captured_read_only_probe_without_the_ssh_agent() -> Checked {
    let host = FakeSbx::listing("[]");
    require_signed_in(&host).required()?;
    let spec = host.spec("ls --json")?;
    assert_eq!(spec.program, "sbx");
    assert_eq!(spec.timeout, TimeoutClass::Probe);
    assert_eq!(spec.env, EnvPolicy::InheritWithoutSshAgent);
    assert_eq!(spec.output(), OutputPolicy::Capture);
    assert_eq!(host.calls().len(), 1);
    Ok(())
}

#[test]
fn missing_executables_are_unobservable_and_do_not_ask_for_login() -> Checked {
    let error = require_signed_in(&FakeHost::new()).refused_because("sbx is not installed")?;
    assert_eq!(error.first_id(), Some(ErrorId::SbxLoginUnobservable));
    assert!(error.diagnostics()[0].remediation.is_none());
    Ok(())
}

struct CanceledHost;

#[test]
fn an_incomplete_warmup_response_is_retried_before_selection() -> Checked {
    let host = FakeSbx::listings(&["{", "[]"]);
    require_signed_in(&host).required()?;
    assert_eq!(host.calls().len(), 2);
    assert!(
        host.specs
            .borrow()
            .iter()
            .all(|spec| spec.timeout == TimeoutClass::Probe)
    );
    Ok(())
}

impl AnsweredHost for CanceledHost {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, _spec: &CommandSpec) -> Result<CommandOutcome> {
        Err(Error::Canceled)
    }
}

#[test]
fn canceling_a_login_probe_stays_canceled() -> Checked {
    let error = require_signed_in(&CanceledHost).refused_because("the user canceled")?;
    assert_eq!(error, Error::Canceled);
    Ok(())
}

struct TimedOutHost;

impl AnsweredHost for TimedOutHost {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let os = ScriptedOs::default()
            .stdout(ScriptedPipe::held_open([]))
            .stderr(ScriptedPipe::held_open([ReadStep::Bytes(
                b"daemon is starting\n",
            )]));
        crate::boundary::host::run_inner(&os, os.clock(), spec, spec.timeout.duration())
    }
}

#[test]
fn a_login_timeout_renders_the_command_limit_and_partial_stderr_in_both_languages() -> Checked {
    let host = TimedOutHost;
    let error = require_signed_in(&host).refused_because("the daemon never finishes")?;
    for (locale, limit, explanation) in [
        (Locale::En, "10 seconds", "sandbox listing failed"),
        (Locale::Ja, "10 秒", "Sandbox一覧の取得に失敗"),
    ] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        Ui::capture(locale, RenderingPolicy::plain(), &mut stdout, &mut stderr).error(&error);
        let text = String::from_utf8(stderr).required()?;
        for expected in [
            "sbx-login-unobservable",
            "sbx ls --json",
            limit,
            explanation,
            "daemon is starting",
            "sbx diagnose",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert!(
            !text.contains("sbx login"),
            "an unobservable session does not require signing in: {text}"
        );
        assert!(stdout.is_empty());
    }
    Ok(())
}

fn require_signed_in(
    host: &dyn crate::boundary::host::HostEnvironment,
) -> Result<Vec<crate::boundary::host::protocol::SandboxEntry>> {
    super::require_signed_in(host, &mut crate::design::SilentProgress)
}
