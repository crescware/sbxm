//! 成果物の診断。
use std::os::unix::fs::PermissionsExt;

use crate::commands::status::project::{ProjectStatus, Value};
use crate::design::Fact;
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;
use crate::metadata;
use crate::paths::{self};
use crate::support::image::{self, LABEL_CANONICAL_ID, LABEL_DOCKERFILE_SHA256};

use crate::testing::outcome::{Checked, Required};

use super::check_directory;
use super::{super::diagnose, super::fake::*};
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, project_id};
use crate::testing::value::IMAGE_ID;

#[test]
fn a_host_clone_is_ready_only_once_it_holds_a_git_directory() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;

    let mut status = bare_status();
    check_directory(&project.paths, &mut status);
    assert_eq!(value_of(&status, "status-item-project-root")?, Value::Ready);
    assert_eq!(value_of(&status, "status-item-host-clone")?, Value::Missing);

    // cloneが済んだことは`.git`の有無でだけ言える。空のdirectoryでは作業できない。
    std::fs::create_dir_all(project.paths.host_clone()).required()?;
    let mut status = bare_status();
    check_directory(&project.paths, &mut status);
    assert_eq!(value_of(&status, "status-item-host-clone")?, Value::Missing);

    std::fs::create_dir_all(project.paths.host_clone().join(".git")).required()?;
    let mut status = bare_status();
    check_directory(&project.paths, &mut status);
    assert_eq!(value_of(&status, "status-item-host-clone")?, Value::Ready);
    assert!(
        status.is_healthy(),
        "reading what is there is never a failure: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_project_root_that_is_not_there_is_reported_as_missing_rather_than_failing() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // 登録は残ったまま案件directoryだけが消えた状態も、診断は答えを返して続ける。
    std::fs::remove_dir_all(project.paths.root()).required()?;

    let mut status = bare_status();
    check_directory(&project.paths, &mut status);
    assert_eq!(
        value_of(&status, "status-item-project-root")?,
        Value::Missing
    );
    assert_eq!(value_of(&status, "status-item-host-clone")?, Value::Missing);
    assert!(
        status.is_healthy(),
        "a project root that is not there is not an error: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn an_engine_that_cannot_be_asked_does_not_make_an_image_absent() -> Checked {
    let fixture = Fixture::new()?;
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
    let fixture = Fixture::new()?;
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

#[test]
fn a_dockerfile_whose_digest_matches_the_recorded_generation_is_ready() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let contents = "FROM scratch\nRUN true\n";
    std::fs::write(project.paths.dockerfile(), contents).required()?;
    // 適用済み世代は、記録したdigestと現在のfileのdigestが一致することでだけ言える。
    let mut metadata = project.metadata.clone();
    metadata.provisioning.dockerfile_sha256 = sha256_hex(contents.as_bytes());
    metadata::update(&project.paths, &metadata).required()?;

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-dockerfile")?, Value::Ready);
    assert!(status.is_healthy(), "{:?}", status.diagnostics);
    Ok(())
}

#[test]
fn a_dockerfile_that_cannot_be_read_is_neither_absent_nor_a_new_generation() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let dockerfile = project.paths.dockerfile();
    std::fs::write(&dockerfile, "FROM scratch\n").required()?;
    // 読めないfileのdigestは求められない。世代を比べずに答えを決めない。
    std::fs::set_permissions(&dockerfile, std::fs::Permissions::from_mode(0o000)).required()?;

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    );
    std::fs::set_permissions(&dockerfile, std::fs::Permissions::from_mode(0o600)).required()?;
    let status = status.required_because("diagnose")?;

    assert_eq!(
        value_of(&status, "status-item-dockerfile")?,
        Value::Mismatch
    );
    assert!(
        names_path(
            &status,
            ErrorId::ProjectPathUnreadable,
            &paths::display(&dockerfile)
        ),
        "the unreadable path is named: {:?}",
        status.diagnostics
    );
    assert!(
        status.diagnostics.iter().any(|diagnostic| {
            diagnostic.id == ErrorId::ProjectPathUnreadable
                && diagnostic.facts.iter().any(|fact| {
                    matches!(fact, Fact::OneLine { label, .. }
                        if label.id == "diagnostic-cause-label")
                })
        }),
        "the reason the read failed is carried with the diagnostic: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_dockerfile_that_is_a_symlink_is_refused_instead_of_followed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let elsewhere = fixture.dir.path().join("elsewhere-Dockerfile");
    std::fs::write(&elsewhere, "FROM scratch\n").required()?;
    let dockerfile = project.paths.dockerfile();
    std::os::unix::fs::symlink(&elsewhere, &dockerfile).required()?;

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &FakeSbx::listing("[]"),
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(
        value_of(&status, "status-item-dockerfile")?,
        Value::Mismatch
    );
    assert!(
        names_path(
            &status,
            ErrorId::ProjectPathSymlink,
            &paths::display(&dockerfile)
        ),
        "the symlink is refused by path, not followed: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn an_image_that_declares_this_project_and_this_generation_is_ready() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let generation = &project.metadata.provisioning.dockerfile_sha256;
    let image = image::image_name(&project.sandbox, generation);
    let host = FakeSbx::listing("[]")
        .answering(&format!("image ls --quiet {image}"), 0, "a3d0f4449170\n")
        .answering(
            &format!("image inspect {image}"),
            0,
            &inspect_output(&[
                (
                    LABEL_CANONICAL_ID,
                    &project.metadata.canonical_id().to_string(),
                ),
                (LABEL_DOCKERFILE_SHA256, generation),
            ]),
        );

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-image")?, Value::Ready);
    assert!(status.is_healthy(), "{:?}", status.diagnostics);
    Ok(())
}

#[test]
fn an_image_whose_labels_declare_something_else_is_unusable_rather_than_ready() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let generation = project.metadata.provisioning.dockerfile_sha256.clone();
    let canonical = project.metadata.canonical_id().to_string();
    let image = image::image_name(&project.sandbox, &generation);
    let other_generation = "0".repeat(64);

    // 名前が一致しても、labelが別の案件や別の世代を指すimageはこの案件のものではない。
    for (declared_project, declared_generation) in [
        ("github.com/example-org/other-repo", generation.as_str()),
        (canonical.as_str(), other_generation.as_str()),
    ] {
        let host = FakeSbx::listing("[]")
            .answering(&format!("image ls --quiet {image}"), 0, "a3d0f4449170\n")
            .answering(
                &format!("image inspect {image}"),
                0,
                &inspect_output(&[
                    (LABEL_CANONICAL_ID, declared_project),
                    (LABEL_DOCKERFILE_SHA256, declared_generation),
                ]),
            );

        let status = diagnose(
            &fixture.location,
            &project_id("example-org/example-repo")?,
            &host,
            &fixture.workspace_root,
        )
        .required_because("diagnose")?;
        assert_eq!(
            value_of(&status, "status-item-image")?,
            Value::Mismatch,
            "{declared_project} {declared_generation} is not this project's image"
        );
        assert!(
            status.diagnostics.iter().any(|diagnostic| {
                diagnostic.id == ErrorId::ImageUnusable
                    && diagnostic.facts.iter().any(|fact| {
                        matches!(fact, Fact::OneLine { label, value }
                            if label.id == "diagnostic-image-label" && value.as_str() == image)
                    })
            }),
            "the image that declares something else is named: {:?}",
            status.diagnostics
        );
    }
    Ok(())
}

/// 項目を1件も持たないstatus。1つの検査だけを見るために使う。
fn bare_status() -> ProjectStatus {
    ProjectStatus {
        project: "example-org/example-repo".to_string(),
        items: Vec::new(),
        worktrees: Vec::new(),
        disk: crate::support::disk::DiskObservation::NotObservedMismatch,
        diagnostics: Vec::new(),
    }
}

/// `docker image inspect`が1件のimageについて返す出力。
fn inspect_output(labels: &[(&str, &str)]) -> String {
    let declared: Vec<String> = labels
        .iter()
        .map(|(key, value)| format!(r#""{key}":"{value}""#))
        .collect();
    format!(
        r#"[{{"Id":"{IMAGE_ID}","Config":{{"Labels":{{{}}}}}}}]"#,
        declared.join(",")
    )
}

/// 指定のerrorが、対象のpathを説明文か事実のどちらかで示しているか。
fn names_path(status: &ProjectStatus, id: ErrorId, path: &str) -> bool {
    status.diagnostics.iter().any(|diagnostic| {
        diagnostic.id == id
            && (diagnostic
                .description
                .args
                .iter()
                .any(|(key, value)| *key == "path" && value == path)
                || diagnostic.facts.iter().any(|fact| {
                    matches!(fact, Fact::OneLine { label, value }
                        if label.id == "diagnostic-path-label" && value.as_str() == path)
                }))
    })
}
