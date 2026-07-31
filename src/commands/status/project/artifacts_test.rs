//! 成果物の診断。
use crate::testing::outcome::{Checked, Required};

use super::super::diagnose;
use super::super::fake::*;
use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::{fixture, project_id};

#[test]
fn an_engine_that_cannot_be_asked_does_not_make_an_image_absent() -> Checked {
    let fixture = fixture()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = without_image(FakeSbx::listing("[]"), &project);

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-image")?, Value::Missing);
    assert!(
        status.diagnostics.is_empty(),
        "an image that is simply not there is not a failure: {:?}",
        status.diagnostics
    );

    let image = image::image_name(
        &project.sandbox,
        &project.metadata.provisioning.dockerfile_sha256,
    );
    let host = FakeSbx::listing("[]").answering(&format!("image ls --quiet {image}"), 1, "");

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-image")?, Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalScopeUnobservable),
        "the engine that could not answer is named: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_changed_dockerfile_is_reported_as_the_next_rebuild_rather_than_a_fault() -> Checked {
    let fixture = fixture()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let host = without_image(FakeSbx::listing("[]"), &project);

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-dockerfile")?, Value::Changed);
    assert!(status.is_healthy(), "a changed Dockerfile is not an error");
    Ok(())
}
