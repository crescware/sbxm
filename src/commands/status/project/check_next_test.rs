use crate::boundary::host::protocol::RootDiskUsage;
use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::design::{RenderingPolicy, Ui};
use crate::diagnostics::{Diagnostic, Error, ErrorId, ExitCode, Result};
use crate::i18n::Locale;
use crate::msg;
use crate::support::disk::DiskObservation;
use crate::support::provisioning::{Observation, ProvisioningState};
use crate::testing::command::outcome_with_stderr;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::{Fixture, diagnose, project_id};

use super::check_next;
use crate::commands::status::project::{ProjectStatus, Value, fake::*};

fn status() -> ProjectStatus {
    ProjectStatus {
        project: "example-org/example-repo".into(),
        items: Vec::new(),
        worktrees: Vec::new(),
        files: Vec::new(),
        storage: Vec::new(),
        disk: DiskObservation::Observed(RootDiskUsage {
            free_kib: 0,
            usable_kib: 1024,
            capacity_percent: 100,
        }),
        diagnostics: Vec::new(),
        next: None,
    }
}

fn observation() -> Observation {
    Observation::new(
        ProvisioningState::Fresh,
        "same".into(),
        "same".into(),
        "same".into(),
    )
}

#[test]
fn all_collected_errors_are_reported_once_and_known_results_are_retained() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let first = Diagnostic::new(
        ErrorId::SandboxCheckUnobservable,
        msg!(
            "error-sandbox-check-unobservable",
            subject = "inside check",
            exit_status = "70"
        ),
    );
    let second = Diagnostic::new(
        ErrorId::HostCommandMissing,
        msg!("error-host-command-missing", program = "git"),
    );
    let mut status = status();
    status.push("status-item-image", Value::Ready);
    status.diagnostics.push(first.clone());
    let mut observation = observation();
    observation.block_all(vec![
        Error::single(first.clone()),
        Error::single(second.clone()),
    ]);
    check_next(&project.metadata, Ok(observation), &mut status).required()?;
    assert_eq!(status.diagnostics, [first, second]);
    assert_eq!(status.items[0].value, Value::Ready);
    assert!(matches!(status.disk, DiskObservation::Observed(usage) if usage.free_kib == 0));
    assert_eq!(status.items[1].item, "status-item-maintenance-check");
    assert_eq!(status.items[1].value, Value::NotObserved);
    assert!(!status.is_healthy());
    assert!(status.next.is_none());
    Ok(())
}

#[test]
fn returned_errors_are_retained_and_cancellation_is_propagated() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let error = Error::new(
        ErrorId::HostCommandMissing,
        msg!("error-host-command-missing", program = "git"),
    );
    let mut status = status();
    check_next(&project.metadata, Err(error.clone()), &mut status).required()?;
    assert_eq!(status.diagnostics, error.diagnostics());
    assert_eq!(
        check_next(&project.metadata, Err(Error::Canceled), &mut status)
            .refused_because("the user canceled")?,
        Error::Canceled
    );
    let mut canceled = observation();
    canceled.block_all(vec![Error::Canceled]);
    assert_eq!(
        check_next(&project.metadata, Ok(canceled), &mut status)
            .refused_because("a collected cancellation is still cancellation")?,
        Error::Canceled
    );
    Ok(())
}

#[test]
fn a_successful_observation_does_not_add_a_failed_check() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let mut status = status();
    check_next(&project.metadata, Ok(observation()), &mut status).required()?;
    assert!(status.items.is_empty());
    assert!(status.is_healthy());
    Ok(())
}

struct FullDiskHost(crate::testing::host::FakeSbx);

impl AnsweredHost for FullDiskHost {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if crate::boundary::host::protocol::split_inside_check(&spec.args).is_some()
            && !spec.args.iter().any(|arg| arg == "-m")
        {
            return Ok(outcome_with_stderr(
                spec,
                125,
                "",
                "sandbox exec: cannot create process: No space left on device\n",
            ));
        }
        self.0.answer(spec)
    }
}

#[test]
fn a_full_disk_inside_check_failure_reaches_the_final_output_in_both_languages() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), b"FROM scratch\n").required()?;
    let host = FullDiskHost(looking_inside(
        &fixture,
        &project,
        &three_entries(&project),
    )?.answering(
        &format!("exec {} -- df -Pk /", project.sandbox),
        0,
        "Filesystem 1024-blocks Used Available Capacity Mounted on\noverlay 1024 1024 0 100% /\n",
    ));
    let status = diagnose(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required()?;
    assert!(matches!(status.disk, DiskObservation::Observed(usage) if usage.free_kib == 0));
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|item| item.id == ErrorId::SandboxCheckUnobservable)
        .required()?;
    assert!(
        diagnostic
            .external
            .as_ref()
            .required()?
            .stderr_text()
            .contains("No space left on device")
    );
    for locale in [Locale::En, Locale::Ja] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = crate::commands::status::print::project(
            &mut Ui::capture(locale, RenderingPolicy::plain(), &mut stdout, &mut stderr),
            &status,
        );
        assert_eq!(code, ExitCode::Failure);
        let text = String::from_utf8(stderr).required()?;
        for expected in [
            "sandbox-check-unobservable",
            "sbx exec",
            "125",
            "No space left on device",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
        assert!(text.contains("<inside_check.sh>"), "{text}");
        assert!(
            !text.contains("# sbxm inside check"),
            "the script is summarized: {text}"
        );
        let text = String::from_utf8(stdout).required()?;
        assert!(text.contains("not-observed"), "{text}");
        assert!(text.contains("running"), "{text}");
        assert!(text.contains("0 MiB"), "{text}");
    }
    Ok(())
}
