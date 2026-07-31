use super::*;
use crate::command::OutputPolicy;
use crate::metadata::{self, RebuildIntent};
use crate::testing::host::{FakeSbx, assert_lifecycle};
use crate::testing::poll::poll;
use crate::testing::project::{fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;

#[test]
fn only_the_running_targets_are_stopped() {
    let fixture = fixture();
    let first = fixture.register("alpha/alfa");
    let second = fixture.register("zeta/zulu");
    let running = format!(
        "[{},{}]",
        fixture.entry(&first, "running"),
        fixture.entry(&second, "stopped")
    );
    let after = format!(
        "[{},{}]",
        fixture.entry(&first, "stopped"),
        fixture.entry(&second, "stopped")
    );
    let host = FakeSbx::listings(&[&running, &running, &after]);

    let report = run(
        &fixture.location,
        &[project_id("zeta/zulu"), project_id("alpha/alfa")],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect("stop");

    assert_eq!(
        report
            .outcomes
            .iter()
            .map(|outcome| (outcome.project.as_str(), outcome.result))
            .collect::<Vec<_>>(),
        vec![
            ("alpha/alfa", StopResult::Stopped),
            ("zeta/zulu", StopResult::Unchanged),
        ],
        "targets are processed in canonical order"
    );
    assert!(report.failures.is_empty());
    assert!(host.ran(&format!("stop {}", first.sandbox)));
    assert!(
        !host.ran(&format!("stop {}", second.sandbox)),
        "a sandbox that is already stopped is left alone"
    );

    assert_lifecycle(&host, &format!("stop {}", first.sandbox));
    assert_eq!(
        host.spec("ls --json").output,
        OutputPolicy::Capture,
        "the state is read from structured output"
    );
}

#[test]
fn a_project_without_a_sandbox_is_a_no_op_success() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");
    let host = FakeSbx::listing("[]");

    let report = run(
        &fixture.location,
        &[project_id("example-org/example-repo")],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect("stop");
    assert_eq!(report.outcomes[0].result, StopResult::Unchanged);
    assert!(!host.ran("stop "));
}

#[test]
fn a_rebuild_in_progress_stops_nothing_at_all() {
    let fixture = fixture();
    let first = fixture.register("alpha/alfa");
    let second = fixture.register("zeta/zulu");
    let mut metadata = second.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&second.paths, &metadata).expect("record the intent");

    let listing = format!(
        "[{},{}]",
        fixture.entry(&first, "running"),
        fixture.entry(&second, "running")
    );
    let host = FakeSbx::listing(&listing);

    let error = run(
        &fixture.location,
        &[project_id("alpha/alfa"), project_id("zeta/zulu")],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("one target that cannot be stopped stops the whole run");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(!host.ran("stop "), "nothing is stopped: {:?}", host.calls());
}

#[test]
fn an_intent_recorded_after_the_first_check_is_still_seen() {
    let fixture = fixture();
    let project = fixture.register("alpha/alfa");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    let host = FakeSbx::listing(&listing);

    // 選択と最初の検査のあとにrebuildが始まった状態を、lock取得後に読み直す。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).expect("record the intent");

    let error = run(
        &fixture.location,
        &[project_id("alpha/alfa")],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("the metadata on disk decides after the lock is held");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(!host.ran("stop "));
}

#[test]
fn a_failure_leaves_the_remaining_targets_running() {
    let fixture = fixture();
    let first = fixture.register("alpha/alfa");
    let second = fixture.register("zeta/zulu");
    let running = format!(
        "[{},{}]",
        fixture.entry(&first, "running"),
        fixture.entry(&second, "running")
    );
    let host = FakeSbx::listing(&running).answering(&format!("stop {}", first.sandbox), 1, "");

    let report = run(
        &fixture.location,
        &[project_id("alpha/alfa"), project_id("zeta/zulu")],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect("the report is produced even when a target fails");

    assert_eq!(report.outcomes[0].result, StopResult::Failed);
    assert_eq!(report.outcomes[1].result, StopResult::Unchanged);
    assert!(!report.failures.is_empty());
    assert!(
        !host.ran(&format!("stop {}", second.sandbox)),
        "the run does not continue past a failure"
    );
}

#[test]
fn a_sandbox_that_stays_running_is_reported_as_failed() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = FakeSbx::listing(&running);

    let report = run(
        &fixture.location,
        &[project_id("example-org/example-repo")],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect("report");
    assert_eq!(report.outcomes[0].result, StopResult::Failed);
    assert!(
        report
            .failures
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxStillRunning)
    );
}

#[test]
fn an_omitted_target_is_chosen_from_the_managed_projects() {
    let fixture = fixture();
    let first = fixture.register("alpha/alfa");
    let second = fixture.register("zeta/zulu");
    let running = format!(
        "[{},{}]",
        fixture.entry(&first, "running"),
        fixture.entry(&second, "running")
    );
    let after = format!(
        "[{},{}]",
        fixture.entry(&first, "stopped"),
        fixture.entry(&second, "running")
    );
    let host = FakeSbx::listings(&[&running, &running, &after]);

    let report = run(
        &fixture.location,
        &[],
        &host,
        &mut ScriptedPrompt::choosing_many(&[0]),
        &fixture.workspace_root,
        poll(),
    )
    .expect("stop");
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(report.outcomes[0].project, "alpha/alfa");
}
