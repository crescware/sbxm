use super::*;
use crate::error::ErrorId;
use crate::paths::display;
use crate::support::inventory::ProjectState;
use crate::testing::host::FakeSbx;
use crate::testing::project::{fixture, ssh_repository};

#[test]
fn managed_projects_and_unmanaged_sandboxes_are_listed_separately() {
    let fixture = fixture();
    let first = fixture.register("Example-Org/Example-Repo");
    let second = fixture.register("other/repo");
    let host = FakeSbx::listing(&format!(
        r#"[{},{},{{"name":"sbxm-foreign","state":"Running","workspace":"/tmp/elsewhere","template":"other:1"}}]"#,
        fixture.entry(&first, "running"),
        fixture.entry(&second, "stopped"),
    ));

    let listing = run(&fixture.location, &host, &fixture.workspace_root).expect("list");
    assert_eq!(
        listing.projects,
        vec![
            ProjectRow {
                project: "Example-Org/Example-Repo".to_string(),
                root: display(first.paths.root()),
                sandbox: first.sandbox.as_str().to_string(),
                observed: Observed::Registered(ProjectState::Running),
            },
            ProjectRow {
                project: "other/repo".to_string(),
                root: display(second.paths.root()),
                sandbox: second.sandbox.as_str().to_string(),
                observed: Observed::Registered(ProjectState::Stopped),
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
}

#[test]
fn an_unmanaged_sandbox_without_a_workspace_shows_a_placeholder() {
    let fixture = fixture();
    let host = FakeSbx::listing(
        r#"[{"name":"sbxm-known","state":"Running","workspace":"/tmp/known","template":"other:1"},{"name":"sbxm-nowhere","state":"Running","template":"other:1"}]"#,
    );

    let listing = run(&fixture.location, &host, &fixture.workspace_root).expect("list");
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
}

#[test]
fn a_project_without_a_sandbox_is_listed_as_not_created() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");
    let listing = run(
        &fixture.location,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    )
    .expect("list");
    assert_eq!(
        listing.projects[0].observed,
        Observed::Registered(ProjectState::NotCreated)
    );
    assert!(listing.settled);
}

#[test]
fn an_entry_whose_artifacts_are_not_there_is_shown_rather_than_dropped() {
    let fixture = fixture();
    let missing = fixture.config.base_path.as_path().join("gone.project");
    fixture.record(&missing, ssh_repository("example-org/gone"));

    let incomplete = fixture.config.base_path.as_path().join("half.project");
    std::fs::create_dir_all(&incomplete).unwrap();
    fixture.record(&incomplete, ssh_repository("example-org/half"));

    let listing = run(
        &fixture.location,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    )
    .expect("list");
    assert_eq!(
        listing
            .projects
            .iter()
            .map(|row| (row.project.as_str(), row.observed.as_str()))
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
}

#[test]
fn a_project_directory_that_names_another_project_is_inconsistent() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // registryはこのrootをexample-org/example-repoのものとして持っている。
    fixture.record(
        &fixture.config.base_path.as_path().join("other.project"),
        ssh_repository("example-org/other"),
    );
    std::fs::create_dir_all(fixture.config.base_path.as_path().join("other.project")).unwrap();
    crate::metadata::create(
        &crate::paths::ProjectPaths::at(
            &fixture.config.base_path.as_path().join("other.project"),
            ssh_repository("example-org/other").canonical_id(),
        ),
        &project.metadata,
    )
    .expect_err("the .sbxm directory is not there yet");

    let paths = crate::paths::ProjectPaths::at(
        &fixture.config.base_path.as_path().join("other.project"),
        ssh_repository("example-org/other").canonical_id(),
    );
    std::fs::create_dir_all(paths.sbxm_dir()).unwrap();
    crate::metadata::create(&paths, &project.metadata).unwrap();

    let listing = run(
        &fixture.location,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    )
    .expect("list");
    let row = listing
        .projects
        .iter()
        .find(|row| row.project == "example-org/other")
        .expect("the entry is shown rather than dropped");
    assert_eq!(row.observed.as_str(), "inconsistent");
    assert!(!listing.settled);
}

#[test]
fn a_host_with_nothing_on_it_still_lists_successfully() {
    let fixture = fixture();
    let listing = run(
        &fixture.location,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    )
    .expect("an empty host is a valid answer");
    assert!(listing.projects.is_empty());
    assert!(listing.unmanaged.is_empty());
}

#[test]
fn a_listing_that_cannot_be_trusted_produces_no_rows() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = FakeSbx::listing(&format!(
        r#"[{{"name":"{}","state":"pausing","workspace":"/tmp/x","template":"x"}}]"#,
        project.sandbox
    ));

    let error = run(&fixture.location, &host, &fixture.workspace_root)
        .expect_err("an unknown state stops the listing");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
}
