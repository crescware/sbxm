use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::support::image::image_name;
use crate::testing::host::FakeSbx;
use crate::testing::project::Fixture;

#[test]
fn projects_and_sandboxes_are_paired_by_exact_name() -> Checked {
    let fixture = Fixture::new()?;
    let first = fixture.register("Example-Org/Example-Repo")?;
    let second = fixture.register("other/other-repo")?;
    let host = FakeSbx::listing(&format!(
        "[{},{}]",
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
    let host = FakeSbx::listing("[]");

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
        r#"[{},{{"name":"sbxm-zeta","state":"running","workspace":"/tmp/elsewhere","template":"other:1"}},{{"name":"sbxm-alpha","state":"stopped","workspace":"/tmp/elsewhere","template":"other:1"}}]"#,
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
        r#"[{{"name":"{}","state":"running","workspace":"/tmp/elsewhere","template":"{}"}}]"#,
        project.sandbox,
        image_name(
            &project.sandbox,
            &project.metadata.provisioning.dockerfile_sha256
        )
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
        "[{},{}]",
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
        r#"[{{"name":"{}","state":"pausing","workspace":"/tmp/x","template":"x"}}]"#,
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
        &FakeSbx::listing("[]"),
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
        r#"[{},{{"name":"sbxm-other","state":"running"}},{{"name":"sbxm-other","state":"stopped"}}]"#,
        fixture.entry(&project, "running")?
    );
    let entries = crate::compatibility::parse_sandbox_list(&listing).required_because("listing")?;

    assert_eq!(
        state_of(&entries, &project.metadata, &fixture.workspace_root).required_because("state")?,
        ProjectState::Running
    );
    Ok(())
}
