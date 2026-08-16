use crate::commands::ls::{ListState, ProjectRow, UnmanagedRow};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;
use crate::paths::display;
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, ssh_repository};

#[test]
fn managed_projects_and_unmanaged_sandboxes_are_listed_separately() -> Checked {
    let fixture = Fixture::new()?;
    let first = fixture.register("Example-Org/Example-Repo")?;
    let second = fixture.register("other/repo")?;
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{},{},{{"name":"sbxm-foreign","status":"Running","workspaces":["/tmp/elsewhere"]}}]}}"#,
        fixture.entry(&first, "running")?,
        fixture.entry(&second, "stopped")?,
    ));

    let listing =
        run(&fixture.location, &host, &fixture.workspace_root).required_because("list")?;
    assert_eq!(
        listing.projects,
        vec![
            ProjectRow {
                project: "Example-Org/Example-Repo".to_string(),
                root: display(first.paths.root()),
                sandbox: first.sandbox.as_str().to_string(),
                state: ListState::Running,
            },
            ProjectRow {
                project: "other/repo".to_string(),
                root: display(second.paths.root()),
                sandbox: second.sandbox.as_str().to_string(),
                state: ListState::Stopped,
            },
        ]
    );
    assert_eq!(
        listing.unmanaged,
        vec![UnmanagedRow {
            sandbox: "sbxm-foreign".to_string(),
            // 管理外Sandboxのstateは、runtimeが示したまま表示する。
            state: "Running".to_string(),
            workspace: "/tmp/elsewhere".to_string(),
        }]
    );
    Ok(())
}

#[test]
fn an_unmanaged_sandbox_without_a_workspace_shows_a_placeholder() -> Checked {
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing(
        r#"{"sandboxes":[{"name":"sbxm-known","status":"Running","workspaces":["/tmp/known"]},{"name":"sbxm-nowhere","status":"Running"}]}"#,
    );

    let listing =
        run(&fixture.location, &host, &fixture.workspace_root).required_because("list")?;
    assert_eq!(
        listing.unmanaged,
        vec![
            UnmanagedRow {
                sandbox: "sbxm-known".to_string(),
                state: "Running".to_string(),
                workspace: "/tmp/known".to_string(),
            },
            UnmanagedRow {
                sandbox: "sbxm-nowhere".to_string(),
                state: "Running".to_string(),
                workspace: "-".to_string(),
            },
        ]
    );
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_is_listed_as_not_created() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let listing = run(
        &fixture.location,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &fixture.workspace_root,
    )
    .required_because("list")?;
    assert_eq!(listing.projects[0].state, ListState::NotCreated);
    assert!(listing.settled);
    Ok(())
}

#[test]
fn an_entry_whose_artifacts_are_not_there_is_shown_rather_than_dropped() -> Checked {
    let fixture = Fixture::new()?;
    let missing = fixture.parent.as_path().join("gone.project");
    fixture.record(&missing, ssh_repository("example-org/gone")?)?;

    let incomplete = fixture.parent.as_path().join("half.project");
    std::fs::create_dir_all(&incomplete).required()?;
    fixture.record(&incomplete, ssh_repository("example-org/half")?)?;

    let listing = run(
        &fixture.location,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &fixture.workspace_root,
    )
    .required_because("list")?;
    assert_eq!(
        listing
            .projects
            .iter()
            .map(|row| (row.project.as_str(), row.state.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("example-org/gone", "missing"),
            ("example-org/half", "incomplete"),
        ]
    );
    assert!(
        !listing.settled,
        "an entry that needs recovery is not a settled listing"
    );
    Ok(())
}

#[test]
fn a_project_directory_that_names_another_project_is_inconsistent() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // registryはこのrootをexample-org/example-repoのものとして持っている。
    fixture.record(
        &fixture.parent.as_path().join("other.project"),
        ssh_repository("example-org/other")?,
    )?;
    std::fs::create_dir_all(fixture.parent.as_path().join("other.project")).required()?;
    crate::metadata::create(
        &crate::paths::ProjectPaths::at(
            &fixture.parent.as_path().join("other.project"),
            ssh_repository("example-org/other")?.canonical_id(),
        ),
        &project.metadata,
    )
    .refused_because("the .sbxm directory is not there yet")?;

    let paths = crate::paths::ProjectPaths::at(
        &fixture.parent.as_path().join("other.project"),
        ssh_repository("example-org/other")?.canonical_id(),
    );
    std::fs::create_dir_all(paths.sbxm_dir()).required()?;
    crate::metadata::create(&paths, &project.metadata).required()?;

    let listing = run(
        &fixture.location,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &fixture.workspace_root,
    )
    .required_because("list")?;
    let row = listing
        .projects
        .iter()
        .find(|row| row.project == "example-org/other")
        .required_because("the entry is shown rather than dropped")?;
    assert_eq!(row.state.as_str(), "inconsistent");
    assert!(!listing.settled);
    Ok(())
}

#[test]
fn a_host_with_nothing_on_it_still_lists_successfully() -> Checked {
    let fixture = Fixture::new()?;
    let listing = run(
        &fixture.location,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &fixture.workspace_root,
    )
    .required_because("an empty host is a valid answer")?;
    assert!(listing.projects.is_empty());
    assert!(listing.unmanaged.is_empty());
    Ok(())
}

#[test]
fn a_listing_that_cannot_be_trusted_produces_no_rows() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo");
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"pausing","workspaces":["/tmp/x"]}}]}}"#,
        project?.sandbox
    ));

    let error = run(&fixture.location, &host, &fixture.workspace_root)
        .refused_because("an unknown state stops the listing")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    Ok(())
}

#[test]
fn a_stopped_project_whose_workspace_is_gone_is_shown_as_open_blocked() -> Checked {
    let fixture = Fixture::new()?;
    let running = fixture.register("example-org/running")?;
    let stopped = fixture.register("example-org/stopped")?;
    // 停止中の案件だけ、runtimeのrecordが残ったままhostのdirectoryが消えている。
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&running, "running")?,
        fixture.declared_entry(&stopped, "stopped"),
    ));

    let listing =
        run(&fixture.location, &host, &fixture.workspace_root).required_because("list")?;
    assert_eq!(
        listing
            .projects
            .iter()
            .map(|row| row.state.as_str())
            .collect::<Vec<_>>(),
        vec!["running", "open-blocked"],
        "the listing shows whether open can proceed directly"
    );
    assert!(
        listing.settled,
        "a missing start-up condition is not a mismatch between the registry and its artifacts"
    );
    Ok(())
}

#[test]
fn a_running_project_is_not_blocked_by_a_missing_workspace() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/running")?;
    let listing = run(
        &fixture.location,
        &FakeSbx::listing(&format!(
            r#"{{"sandboxes":[{}]}}"#,
            fixture.declared_entry(&project, "running")
        )),
        &fixture.workspace_root,
    )
    .required_because("list")?;
    assert_eq!(listing.projects[0].state, ListState::Running);
    Ok(())
}

#[test]
fn a_workspace_that_cannot_be_observed_is_not_reported_as_absent() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    fixture.workspace_is_unobservable(&project)?;
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.declared_entry(&project, "stopped")
    ));

    let listing =
        run(&fixture.location, &host, &fixture.workspace_root).required_because("list")?;
    assert_eq!(listing.projects[0].state, ListState::NotObserved);
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_has_no_workspace_to_ask_about() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let listing = run(
        &fixture.location,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &fixture.workspace_root,
    )
    .required_because("list")?;
    assert_eq!(listing.projects[0].state, ListState::NotCreated);
    Ok(())
}

#[test]
fn an_entry_that_needs_recovery_is_not_asked_about_its_workspace() -> Checked {
    let fixture = Fixture::new()?;
    // project rootが失われた案件は、どのSandboxのrecordを見るべきかも決まらない。
    fixture.record(
        &fixture.parent.as_path().join("gone.project"),
        ssh_repository("example-org/gone")?,
    )?;

    let listing = run(
        &fixture.location,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &fixture.workspace_root,
    )
    .required_because("list")?;
    assert_eq!(listing.projects[0].state.as_str(), "missing");
    Ok(())
}
