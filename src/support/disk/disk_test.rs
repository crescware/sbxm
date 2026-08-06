use crate::compatibility::RootDiskUsage;
use crate::support::inventory::ProjectState;
use crate::testing::host::FakeSbx;
use crate::testing::sandbox::InnerCommandSandbox;

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
        assert_eq!(observe(&host, NAME, state), expected);
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
        observe(&host, NAME, Some(ProjectState::Running)),
        DiskObservation::Observed(RootDiskUsage {
            free_kib: 4_898_320,
            usable_kib: 19_401_296,
            capacity_percent: 75,
        })
    );
    assert_eq!(host.calls().len(), 1);
}

#[test]
fn df_missing_inside_the_sandbox_is_reported_as_missing_not_as_a_parse_failure() {
    // `sbx exec`は内側のcommandを起動できなかったことを125..=127で示す。
    let host = FakeSbx::listing("[]").answering(&format!("exec {NAME} -- df -Pk /"), 127, "");
    assert_eq!(
        observe(&host, NAME, Some(ProjectState::Running)),
        DiskObservation::CommandMissing
    );
}

#[test]
fn df_that_ran_but_exited_non_zero_is_a_parse_failure_not_missing() {
    let host = InnerCommandSandbox::new().failing("df -Pk /");
    assert_eq!(
        observe(&host, NAME, Some(ProjectState::Running)),
        DiskObservation::ParseFailed
    );
}

#[test]
fn a_command_that_never_answers_is_reported_as_unobserved_rather_than_aborting_status() {
    let host = InnerCommandSandbox::new().timing_out("df -Pk /");
    assert_eq!(
        observe(&host, NAME, Some(ProjectState::Running)),
        DiskObservation::ParseFailed
    );
}

#[test]
fn a_successful_but_unparseable_answer_is_a_parse_failure() {
    let host = InnerCommandSandbox::new().answering("df -Pk /", "not df output at all\n");
    assert_eq!(
        observe(&host, NAME, Some(ProjectState::Running)),
        DiskObservation::ParseFailed
    );
}
