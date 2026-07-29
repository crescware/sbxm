use super::*;
use crate::error::ErrorId;
use crate::testing::host::FakeSbx;
use crate::testing::project::fixture;

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

    let listing = run(&fixture.config, &host, &fixture.workspace_root).expect("list");
    assert_eq!(
        listing.projects,
        vec![
            ProjectRow {
                project: "Example-Org/Example-Repo".to_string(),
                sandbox: first.sandbox.as_str().to_string(),
                state: ProjectState::Running,
            },
            ProjectRow {
                project: "other/repo".to_string(),
                sandbox: second.sandbox.as_str().to_string(),
                state: ProjectState::Stopped,
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
fn a_project_without_a_sandbox_is_listed_as_not_created() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");
    let listing = run(
        &fixture.config,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    )
    .expect("list");
    assert_eq!(listing.projects[0].state, ProjectState::NotCreated);
}

#[test]
fn a_host_with_nothing_on_it_still_lists_successfully() {
    let fixture = fixture();
    let listing = run(
        &fixture.config,
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

    let error = run(&fixture.config, &host, &fixture.workspace_root)
        .expect_err("an unknown state stops the listing");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
}
