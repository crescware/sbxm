use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{fake::*, *};
use crate::support::provisioning::NextAction;
use crate::testing::add_request::{project_of, request};
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, project_id};
use crate::testing::provisioning::{Bench, World};

#[test]
fn a_project_that_is_not_managed_cannot_be_diagnosed() -> Checked {
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);
    let error = diagnose(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .refused_because("there is nothing to diagnose")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    Ok(())
}

#[test]
fn the_items_are_reported_in_the_documented_order() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = without_image(FakeSbx::listing(r#"{"sandboxes":[]}"#), &project);

    let status = diagnose(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(
        status
            .items
            .iter()
            .map(|item| item.item)
            .collect::<Vec<_>>(),
        vec![
            "status-item-metadata",
            "status-item-project-root",
            "status-item-host-clone",
            "status-item-dockerfile",
            "status-item-image",
            "status-item-sandbox",
            "status-item-workspace",
            "status-item-secret",
            "status-item-bare-repository",
            "status-item-worktrees",
            "status-item-ssh-agent",
        ]
    );
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_reports_the_inner_items_as_not_applicable() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("Example-Org/Example-Repo")?;
    let host = without_image(FakeSbx::listing(r#"{"sandboxes":[]}"#), &project);

    let status = diagnose(
        &fixture.location,
        &fixture.config,
        &project_id("Example-Org/Example-Repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(status.project, "Example-Org/Example-Repo");
    assert_eq!(value_of(&status, "status-item-metadata")?, Value::Ready);
    assert_eq!(value_of(&status, "status-item-sandbox")?, Value::NotCreated);
    for item in [
        // Sandboxが無い案件には、mount元のworkspaceも、中で見る対象も無い。
        "status-item-workspace",
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(value_of(&status, item)?, Value::NotApplicable, "{item}");
    }
    assert!(status.worktrees.is_empty());
    Ok(())
}

#[test]
fn an_unfinished_first_provisioning_is_named_with_the_command_that_recovers_it() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the build is interrupted after its intent was saved")?;
    world.nothing_fails();

    let status = diagnose(
        &bench.location,
        &bench.config,
        &project_of(&request)?,
        &world,
        bench.workspace_root.path(),
    )
    .required_because("diagnose the interrupted project")?;

    // 判定は`repair`と同じ共有観測から来る。statusが別の規則で結論を出さない。
    assert_eq!(status.next, Some(NextAction::OpenPending));
    assert!(
        !status.is_healthy(),
        "an unfinished first provisioning does not end successfully"
    );
    Ok(())
}

#[test]
fn a_changed_dockerfile_on_a_finished_project_is_named_as_a_generation_change() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    let project = project_of(&request)?;
    let candidate = crate::support::select::find(&bench.location, &project)
        .required_because("find the built project")?;
    std::fs::write(
        candidate.paths.dockerfile(),
        b"FROM example\nRUN echo new\n",
    )
    .required_because("edit the Dockerfile after the build")?;

    let status = diagnose(
        &bench.location,
        &bench.config,
        &project,
        &world,
        bench.workspace_root.path(),
    )
    .required_because("diagnose the finished project")?;

    assert_eq!(status.next, Some(NextAction::RebuildChanged));
    Ok(())
}

#[test]
fn a_stopped_project_is_not_given_a_command_that_cannot_be_proven() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    // 停止中のSandboxの中は読めない。欠けているとも揃っているとも言えない案件へ、
    // 実行できると証明できないcommandを出さない。
    world.stopped();
    let status = diagnose(
        &bench.location,
        &bench.config,
        &project_of(&request)?,
        &world,
        bench.workspace_root.path(),
    )
    .required_because("diagnose the stopped project")?;

    assert_eq!(status.next, None);
    Ok(())
}

#[test]
fn a_secret_not_applied_to_the_sandbox_is_not_given_a_doomed_repair() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;
    for sandbox in world.sandboxes.borrow_mut().iter_mut() {
        sandbox.placeholder = false;
    }

    let status = diagnose(
        &bench.location,
        &bench.config,
        &project_of(&request)?,
        &world,
        bench.workspace_root.path(),
    )
    .required_because("diagnose the sandbox that never received its secret")?;

    assert_eq!(status.next, None);
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == ErrorId::SandboxSecretNotApplied)
        .required_because("the actionable secret diagnostic remains")?;
    let commands = diagnostic
        .remediation
        .as_ref()
        .required_because("the secret diagnostic names its recovery command")?
        .commands
        .iter()
        .map(crate::design::text::CommandLine::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        vec![format!(
            "sbx rm {}",
            crate::project::SandboxName::derive(request.repository.canonical_id())
        )]
    );
    Ok(())
}
