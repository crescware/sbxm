use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, OutputPolicy, TimeoutClass,
};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::testing::global_status::FakeHost;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};

use super::require_signed_in;

#[test]
fn login_is_checked_with_a_captured_read_only_probe_without_the_ssh_agent() -> Checked {
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);
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
    let host = FakeSbx::listings(&["{", r#"{"sandboxes":[]}"#]);
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

impl HostEnvironment for CanceledHost {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, _spec: &CommandSpec) -> Result<CommandOutcome> {
        Err(Error::Canceled)
    }
}

#[test]
fn canceling_a_login_probe_stays_canceled() -> Checked {
    let error = require_signed_in(&CanceledHost).refused_because("the user canceled")?;
    assert_eq!(error, Error::Canceled);
    Ok(())
}
