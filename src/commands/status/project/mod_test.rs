use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{fake::*, *};
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, project_id};

#[test]
fn a_project_that_is_not_managed_cannot_be_diagnosed() -> Checked {
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);
    let error = diagnose(
        &fixture.location,
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
            "status-item-template-archive",
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
        &project_id("Example-Org/Example-Repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(status.project, "Example-Org/Example-Repo");
    assert_eq!(value_of(&status, "status-item-metadata")?, Value::Ready);
    assert_eq!(value_of(&status, "status-item-sandbox")?, Value::NotCreated);
    for item in [
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
