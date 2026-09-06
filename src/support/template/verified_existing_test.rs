use std::fs;

use crate::diagnostics::ErrorId;
use crate::support::image::BuiltImage;
use crate::testing::archive::image_archive_bytes;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};

use super::verified_existing;

const IMAGE_NAME: &str = "sbxm-example-template:aaaaaaaaaaaa";
const MATCHING_ID: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn built_image() -> BuiltImage {
    BuiltImage {
        name: IMAGE_NAME.to_string(),
        id: MATCHING_ID.to_string(),
        labels: Vec::new(),
        built: false,
        warnings: Vec::new(),
    }
}

fn archive_path(dir: &std::path::Path) -> Checked<std::path::PathBuf> {
    let path = dir.join("archive.tar");
    fs::write(&path, image_archive_bytes(IMAGE_NAME, MATCHING_ID, &[]))
        .required_because("write the test archive")?;
    Ok(path)
}

#[test]
fn a_template_with_no_matching_name_is_not_reused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering("template ls --json", 0, r#"{"images":[]}"#);
    let loaded = verified_existing(&host, &built_image(), &archive_path(dir.path())?)
        .required_because("an absent template is observed, not an error")?;
    assert!(loaded.is_none());
    Ok(())
}

#[test]
fn a_template_whose_runtime_id_matches_the_verified_image_is_reused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering(
        "template ls --json",
        0,
        r#"{"images":[{"id":"aaaaaaaaaaaa","repository":"docker.io/library/sbxm-example-template","tag":"aaaaaaaaaaaa"}]}"#,
    );
    let loaded = verified_existing(&host, &built_image(), &archive_path(dir.path())?)
        .required_because("a matching runtime id is reused")?;
    assert_eq!(
        loaded.map(|template| template.name),
        Some(IMAGE_NAME.to_string())
    );
    Ok(())
}

#[test]
fn a_template_whose_runtime_id_differs_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering(
        "template ls --json",
        0,
        r#"{"images":[{"id":"deadbeefdead","repository":"docker.io/library/sbxm-example-template","tag":"aaaaaaaaaaaa"}]}"#,
    );
    let error = verified_existing(&host, &built_image(), &archive_path(dir.path())?)
        .refused_because("a different runtime id is not the verified image")?;
    assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    Ok(())
}

#[test]
fn a_template_with_no_reported_id_is_refused_as_unobservable() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering(
        "template ls --json",
        0,
        r#"{"images":[{"repository":"docker.io/library/sbxm-example-template","tag":"aaaaaaaaaaaa"}]}"#,
    );
    let error = verified_existing(&host, &built_image(), &archive_path(dir.path())?)
        .refused_because("no id means the correspondence cannot be confirmed")?;
    assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    Ok(())
}
