use std::os::unix::fs::PermissionsExt;

use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::Fixture;

#[test]
fn projects_and_sandboxes_are_paired_by_exact_name() -> Checked {
    let fixture = Fixture::new()?;
    let first = fixture.register("Example-Org/Example-Repo")?;
    let second = fixture.register("other/other-repo")?;
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&first, "running")?,
        fixture.entry(&second, "stopped")?
    ));

    let inventory =
        take(&fixture.location, &host, &fixture.workspace_root).required_because("inventory")?;
    assert_eq!(
        inventory
            .projects
            .iter()
            .map(|project| (project.display_id.clone(), project.observed.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                "Example-Org/Example-Repo".to_string(),
                Observed::Registered(ProjectState::Running)
            ),
            (
                "other/other-repo".to_string(),
                Observed::Registered(ProjectState::Stopped)
            ),
        ],
        "projects are listed in canonical order"
    );
    assert!(inventory.unmanaged.is_empty());
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_is_not_created_rather_than_missing() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let inventory =
        take(&fixture.location, &host, &fixture.workspace_root).required_because("inventory")?;
    assert_eq!(
        inventory.projects[0].observed,
        Observed::Registered(ProjectState::NotCreated)
    );
    assert_eq!(inventory.projects[0].observed.as_str(), "not-created");
    Ok(())
}

#[test]
fn a_sandbox_that_belongs_to_no_project_is_listed_separately() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{},{{"name":"sbxm-zeta","status":"running","workspaces":["/tmp/elsewhere"]}},{{"name":"sbxm-alpha","status":"stopped","workspaces":["/tmp/elsewhere"]}}]}}"#,
        fixture.entry(&project, "running")?
    ));

    let inventory =
        take(&fixture.location, &host, &fixture.workspace_root).required_because("inventory")?;
    assert_eq!(inventory.projects.len(), 1);
    assert_eq!(
        inventory
            .unmanaged
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["sbxm-alpha", "sbxm-zeta"],
        "unmanaged sandboxes are listed by name"
    );
    Ok(())
}

#[test]
fn an_inconsistent_pairing_is_refused_rather_than_reported_as_a_state() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"running","workspaces":["/tmp/elsewhere"]}}]}}"#,
        project.sandbox,
    ));

    let error = take(&fixture.location, &host, &fixture.workspace_root)
        .refused_because("a sandbox that works elsewhere is not this project's")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    Ok(())
}

#[test]
fn a_listing_that_cannot_be_paired_stops_before_anything_is_shown() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;

    // 同名のSandboxが2件ある一覧からは対応を決められない。
    let duplicated = format!(
        r#"{{"sandboxes":[{},{}]}}"#,
        fixture.entry(&project, "running")?,
        fixture.entry(&project, "stopped")?
    );
    let error = take(
        &fixture.location,
        &FakeSbx::listing(&duplicated),
        &fixture.workspace_root,
    )
    .refused_because("duplicate names are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNameCollision));

    // 未対応のraw stateも同じく一覧を成立させない。
    let unknown = format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"pausing","workspaces":["/tmp/x"]}}]}}"#,
        project.sandbox
    );
    let error = take(
        &fixture.location,
        &FakeSbx::listing(&unknown),
        &fixture.workspace_root,
    )
    .refused_because("an unknown state is not rounded to a known one")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));

    // 読めないmetadataは、そのentryだけを`inconsistent`として示す。ほかのentryは
    // 一覧から消さない。
    let broken = fixture.parent.as_path().join("broken.project");
    std::fs::create_dir_all(broken.join(".sbxm")).required()?;
    std::fs::write(broken.join(".sbxm").join("project.yaml"), "version: 2\n").required()?;
    fixture.record(
        &broken,
        crate::testing::project::ssh_repository("org/broken")?,
    )?;

    let inventory = take(
        &fixture.location,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &fixture.workspace_root,
    )
    .required_because("a broken project does not take the listing down with it")?;
    assert_eq!(
        inventory
            .projects
            .iter()
            .map(|project| (project.display_id.as_str(), project.observed.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("example-org/example-repo", "not-created"),
            ("org/broken", "inconsistent"),
        ]
    );
    assert!(!inventory.is_settled());
    Ok(())
}

#[test]
fn one_project_is_resolved_without_the_rest_of_the_listing_being_sound() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // 別のSandboxが2件同名でも、この案件の対応は名前の完全一致で決まる。
    let listing = format!(
        r#"{{"sandboxes":[{},{{"name":"sbxm-other","status":"running"}},{{"name":"sbxm-other","status":"stopped"}}]}}"#,
        fixture.entry(&project, "running")?
    );
    let entries = crate::compatibility::parse_sandbox_list(&listing).required_because("listing")?;

    assert_eq!(
        state_of(&entries, &project.metadata, &fixture.workspace_root).required_because("state")?,
        ProjectState::Running
    );
    Ok(())
}

#[test]
fn observed_states_keep_their_spelling_legend_and_settled_meaning() {
    for (observed, spelling, legend) in [
        (Observed::Missing, "missing", "legend-missing"),
        (Observed::Incomplete, "incomplete", "legend-incomplete"),
        (
            Observed::Inconsistent,
            "inconsistent",
            "legend-inconsistent",
        ),
    ] {
        assert_eq!(observed.as_str(), spelling);
        assert_eq!(observed.legend_id(), legend);
        assert!(!observed.is_settled());
    }

    let registered = Observed::Registered(ProjectState::Running);
    assert_eq!(registered.as_str(), "running");
    assert_eq!(registered.legend_id(), "legend-sandbox-running");
    assert!(registered.is_settled());
}

#[test]
fn single_refuses_two_entries_with_the_requested_name() -> Checked {
    let entries = vec![
        crate::compatibility::SandboxEntry {
            name: "sbxm-example".to_string(),
            state: crate::compatibility::SandboxState::Running,
            raw_state: "running".to_string(),
            workspace: None,
        },
        crate::compatibility::SandboxEntry {
            name: "sbxm-example".to_string(),
            state: crate::compatibility::SandboxState::Stopped,
            raw_state: "stopped".to_string(),
            workspace: None,
        },
    ];

    let error =
        single(&entries, "sbxm-example").refused_because("two matching sandboxes are ambiguous")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNameCollision));
    Ok(())
}

#[test]
fn a_stopped_sandbox_whose_workspace_is_gone_is_not_asked_to_start() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // runtimeのrecordは残っているが、mount元のdirectoryはhostから消えている。
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.declared_entry(&project, "stopped")
    ));

    let error = start(
        &host,
        &project.metadata,
        &fixture.workspace_root,
        &mut crate::design::SilentProgress,
    )
    .refused_because("the runtime refuses to start a sandbox whose workspace is gone")?;

    assert_eq!(error.first_id(), Some(ErrorId::SandboxWorkspaceMissing));
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-sandbox-workspace-missing"),
        "the refusal names how to restore the directory"
    );
    assert!(
        !host.ran("/bin/true"),
        "the start is refused before the runtime is asked: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_workspace_that_is_present_lets_the_start_go_through() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "stopped")?
    ));

    start(
        &host,
        &project.metadata,
        &fixture.workspace_root,
        &mut crate::design::SilentProgress,
    )
    .required_because("start")?;

    assert!(
        host.ran("/bin/true"),
        "the sandbox is started once its workspace is observed: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_workspace_that_another_account_could_write_to_is_not_trusted_for_a_start() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing(&format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "stopped")?
    ));

    // 実在はするが、他accountも書き込めるpermissionのままになっている。cleanupの
    // あとで別accountが用意した可能性を、実在の確認だけでは除けない。
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o777))
        .required_because("widen the permission")?;

    let error = start(
        &host,
        &project.metadata,
        &fixture.workspace_root,
        &mut crate::design::SilentProgress,
    )
    .refused_because("an open workspace is not trusted before the start is asked")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert!(
        !host.ran("/bin/true"),
        "the start is refused before the runtime is asked: {:?}",
        host.calls()
    );
    Ok(())
}
