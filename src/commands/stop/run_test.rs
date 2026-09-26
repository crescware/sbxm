use crate::commands::stop::StopResult;
use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;
use crate::boundary::host::OutputPolicy;
use crate::metadata::{self, RebuildIntent};
use crate::testing::host::{FakeSbx, assert_lifecycle};
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;

#[test]
fn only_the_running_targets_are_stopped() -> Checked {
    let fixture = Fixture::new()?;
    let first = fixture.register("alpha/alfa")?;
    let second = fixture.register("zeta/zulu")?;
    let running = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&first, "running")?,
        fixture.entry(&second, "stopped")?
    );
    let after = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&first, "stopped")?,
        fixture.entry(&second, "stopped")?
    );
    let host = FakeSbx::listings(&[&running, &running, &after]);

    let report = run(
        &fixture.location,
        &[project_id("zeta/zulu")?, project_id("alpha/alfa")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .required_because("stop")?;

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

    assert_lifecycle(&host, &format!("stop {}", first.sandbox))?;
    assert_eq!(
        host.spec("ls --json")?.output(),
        OutputPolicy::Capture,
        "the state is read from structured output"
    );
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_is_a_no_op_success() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let report = run(
        &fixture.location,
        &[project_id("example-org/example-repo")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .required_because("stop")?;
    assert_eq!(report.outcomes[0].result, StopResult::Unchanged);
    assert!(!host.ran("stop "));
    Ok(())
}

#[test]
fn a_rebuild_in_progress_stops_nothing_at_all() -> Checked {
    let fixture = Fixture::new()?;
    let first = fixture.register("alpha/alfa")?;
    let second = fixture.register("zeta/zulu")?;
    let mut metadata = second.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&second.paths, &metadata).required_because("record the intent")?;

    let listing = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&first, "running")?,
        fixture.entry(&second, "running")?
    );
    let host = FakeSbx::listing(&listing);

    let error = run(
        &fixture.location,
        &[project_id("alpha/alfa")?, project_id("zeta/zulu")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .refused_because("one target that cannot be stopped stops the whole run")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(!host.ran("stop "), "nothing is stopped: {:?}", host.calls());
    Ok(())
}

#[test]
fn an_intent_recorded_after_the_first_check_is_still_seen() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("alpha/alfa")?;
    let listing = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
    let host = FakeSbx::listing(&listing);

    // 選択と最初の検査のあとにrebuildが始まった状態を、lock取得後に読み直す。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required_because("record the intent")?;

    let error = run(
        &fixture.location,
        &[project_id("alpha/alfa")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .refused_because("the metadata on disk decides after the lock is held")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(!host.ran("stop "));
    Ok(())
}

#[test]
fn a_failure_leaves_the_remaining_targets_running() -> Checked {
    let fixture = Fixture::new()?;
    let first = fixture.register("alpha/alfa")?;
    let second = fixture.register("zeta/zulu")?;
    let running = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&first, "running")?,
        fixture.entry(&second, "running")?
    );
    let host = FakeSbx::listing(&running).answering(&format!("stop {}", first.sandbox), 1, "");

    let report = run(
        &fixture.location,
        &[project_id("alpha/alfa")?, project_id("zeta/zulu")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .required_because("the report is produced even when a target fails")?;

    assert_eq!(report.outcomes[0].result, StopResult::Failed);
    assert_eq!(report.outcomes[1].result, StopResult::Unchanged);
    assert!(!report.failures.is_empty());
    assert!(
        !host.ran(&format!("stop {}", second.sandbox)),
        "the run does not continue past a failure"
    );
    Ok(())
}

#[test]
fn a_sandbox_that_stays_running_is_reported_as_failed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
    let host = FakeSbx::listing(&running);

    let report = run(
        &fixture.location,
        &[project_id("example-org/example-repo")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .required_because("report")?;
    assert_eq!(report.outcomes[0].result, StopResult::Failed);
    assert!(
        report
            .failures
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxStillRunning)
    );
    Ok(())
}

#[test]
fn an_omitted_target_is_chosen_from_the_managed_projects() -> Checked {
    let fixture = Fixture::new()?;
    let first = fixture.register("alpha/alfa")?;
    let second = fixture.register("zeta/zulu")?;
    let running = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&first, "running")?,
        fixture.entry(&second, "running")?
    );
    let after = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&first, "stopped")?,
        fixture.entry(&second, "running")?
    );
    let host = FakeSbx::listings(&[&running, &running, &after]);

    let report = run(
        &fixture.location,
        &[],
        &host,
        &mut ScriptedPrompt::choosing_many(&[0]),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .required_because("stop")?;
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(report.outcomes[0].project, "alpha/alfa");
    Ok(())
}

#[test]
fn a_running_local_sandbox_is_asked_for_its_commits_before_it_stops() -> Checked {
    let fixture = Fixture::new()?;
    let local = fixture.register_local("/srv/code/app/.git", "app")?;
    let github = fixture.register("zeta/zulu")?;
    let running = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&local, "running")?,
        fixture.entry(&github, "running")?
    );
    let after = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&local, "stopped")?,
        fixture.entry(&github, "stopped")?
    );
    // 保存の前に動いていることを確かめる一覧も読む。
    let host = FakeSbx::listings(&[&running, &running, &running, &after]);
    let mut output = RecordedOutput::new();

    let report = run(
        &fixture.location,
        &[project_id("local/app")?, project_id("zeta/zulu")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut output,
    )
    .required_because("stop")?;

    assert_eq!(report.saved.len(), 2, "one save attempt per running target");
    // 保存のあいだ黙って待たせない。
    assert_eq!(
        output
            .steps
            .iter()
            .filter(|step| step.id == "progress-saving-to-host")
            .count(),
        1,
        "{:?}",
        output.steps
    );
    let calls: Vec<String> = host.calls().iter().map(|call| call.join(" ")).collect();
    let saving = calls
        .iter()
        .position(|call| {
            call.contains(&format!("exec {}", local.sandbox))
                && call.contains(crate::support::host_sync::PLACE_SAVE_REFS)
        })
        .required_because("the local sandbox is asked for its commits")?;
    let stopping = calls
        .iter()
        .position(|call| call.contains(&format!("stop {}", local.sandbox)))
        .required_because("the local sandbox is stopped")?;
    assert!(saving < stopping, "{calls:?}");
    // GitHubの案件は、originへpushした作業を持つ。hostへは保存しない。
    assert!(
        !calls
            .iter()
            .any(|call| call.contains(&format!("exec {}", github.sandbox))),
        "{calls:?}"
    );
    Ok(())
}

/// 保存しているあいだに、別の案件のproject lockを取れるかを確かめるhost。
struct ProbingLock {
    inner: FakeSbx,
    lock: std::path::PathBuf,
    free: std::cell::Cell<Option<bool>>,
}

impl crate::boundary::host::HostEnvironment for ProbingLock {
    fn command_exists(&self, program: &str) -> bool {
        self.inner.command_exists(program)
    }

    fn run(
        &self,
        spec: &crate::boundary::host::CommandSpec,
    ) -> crate::diagnostics::Result<crate::boundary::host::CommandOutcome> {
        if spec
            .args
            .iter()
            .any(|arg| arg == crate::support::host_sync::PLACE_SAVE_REFS)
        {
            let taken = crate::paths::acquire_exclusive_lock(
                &self.lock,
                std::time::Duration::from_millis(10),
                crate::paths::PRIVATE_FILE_MODE,
                crate::paths::PathScope::ProjectPath,
            );
            self.free.set(Some(taken.is_ok()));
        }
        self.inner.run(spec)
    }
}

#[test]
fn saving_one_target_leaves_the_others_free_for_other_commands() -> Checked {
    // 保存は時間がかかる。そのあいだ、止める対象すべてのlockを持ち続けない。
    let fixture = Fixture::new()?;
    let local = fixture.register_local("/srv/code/app/.git", "app")?;
    let github = fixture.register("zeta/zulu")?;
    let running = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&local, "running")?,
        fixture.entry(&github, "running")?
    );
    let after = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&local, "stopped")?,
        fixture.entry(&github, "stopped")?
    );
    let host = ProbingLock {
        inner: FakeSbx::listings(&[&running, &running, &running, &after]),
        lock: github.paths.lock_file(),
        free: std::cell::Cell::new(None),
    };

    run(
        &fixture.location,
        &[project_id("local/app")?, project_id("zeta/zulu")?],
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut RecordedOutput::new(),
    )
    .required_because("stop")?;

    assert_eq!(host.free.get(), Some(true), "{:?}", host.inner.calls());
    Ok(())
}
