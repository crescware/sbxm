use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment, TimeoutClass};
use crate::compatibility::RootDiskUsage;
use crate::diagnostics::Result;
use crate::support::inventory::ProjectState;
use crate::testing::host::FakeSbx;
use crate::testing::sandbox::InnerCommandSandbox;
use std::os::unix::process::ExitStatusExt;

use super::*;

const NAME: &str = "sbxm-example-org-example-repo-abc123";

#[test]
fn a_stopped_or_not_created_or_unobservable_sandbox_is_never_started_to_check_disk() {
    let cases = [
        (
            Some(ProjectState::NotCreated),
            DiskObservation::NotObservedNotCreated,
        ),
        (
            Some(ProjectState::Stopped),
            DiskObservation::NotObservedStopped,
        ),
        (None, DiskObservation::NotObservedMismatch),
    ];
    for (state, expected) in cases {
        let host = InnerCommandSandbox::new();
        assert_eq!(
            observe(&host, NAME, state, TimeoutClass::SandboxLifecycle),
            expected
        );
        assert!(
            host.calls().is_empty(),
            "observing disk for {state:?} must not run any command"
        );
    }
}

#[test]
fn a_running_sandbox_is_read_with_a_single_df_call() {
    let host = InnerCommandSandbox::new().answering(
        "df -Pk /",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n",
    );
    assert_eq!(
        observe(
            &host,
            NAME,
            Some(ProjectState::Running),
            TimeoutClass::SandboxLifecycle
        ),
        DiskObservation::Observed(RootDiskUsage {
            free_kib: 4_898_320,
            usable_kib: 19_401_296,
            capacity_percent: 75,
        })
    );
    assert_eq!(host.calls().len(), 1);
}

#[test]
fn only_raw_127_is_reported_as_command_missing() {
    for (code, expected) in [
        (125, DiskObservation::ParseFailed),
        (126, DiskObservation::ParseFailed),
        (127, DiskObservation::CommandMissing),
    ] {
        let host = FakeSbx::listing("[]").answering(&format!("exec {NAME} -- df -Pk /"), code, "");
        assert_eq!(
            observe(
                &host,
                NAME,
                Some(ProjectState::Running),
                TimeoutClass::SandboxLifecycle
            ),
            expected,
            "raw status {code}"
        );
    }
}

struct SignaledSandbox;

impl HostEnvironment for SignaledSandbox {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let mut outcome = crate::testing::command::outcome(spec, 0, "");
        outcome.status = std::process::ExitStatus::from_raw(9);
        Ok(outcome)
    }
}

#[test]
fn a_signaled_command_is_reported_as_a_parse_failure_not_as_missing() {
    assert_eq!(
        observe(
            &SignaledSandbox,
            NAME,
            Some(ProjectState::Running),
            TimeoutClass::SandboxLifecycle
        ),
        DiskObservation::ParseFailed
    );
}

#[test]
fn df_that_ran_but_exited_non_zero_is_a_parse_failure_not_missing() {
    let host = InnerCommandSandbox::new().failing("df -Pk /");
    assert_eq!(
        observe(
            &host,
            NAME,
            Some(ProjectState::Running),
            TimeoutClass::SandboxLifecycle
        ),
        DiskObservation::ParseFailed
    );
}

#[test]
fn a_command_that_never_answers_is_reported_as_unobserved_rather_than_aborting_status() {
    let host = InnerCommandSandbox::new().timing_out("df -Pk /");
    assert_eq!(
        observe(
            &host,
            NAME,
            Some(ProjectState::Running),
            TimeoutClass::SandboxLifecycle
        ),
        DiskObservation::ParseFailed
    );
}

#[test]
fn a_successful_but_unparseable_answer_is_a_parse_failure() {
    let host = InnerCommandSandbox::new().answering("df -Pk /", "not df output at all\n");
    assert_eq!(
        observe(
            &host,
            NAME,
            Some(ProjectState::Running),
            TimeoutClass::SandboxLifecycle
        ),
        DiskObservation::ParseFailed
    );
}
