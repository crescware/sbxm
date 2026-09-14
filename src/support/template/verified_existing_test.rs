use std::fs;

use crate::archive::{manifest_json, tar_bytes};
use crate::diagnostics::ErrorId;
use crate::support::image::BuiltImage;
use crate::testing::archive::{image_archive_bytes, index_id};
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};

use super::verified_existing;

const IMAGE_NAME: &str = "sbxm-example-template:aaaaaaaaaaaa";
/// image configのdigest。legacy layoutのarchiveをloadしたruntimeが報告するid。
const MATCHING_ID: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// `index_id(MATCHING_ID)`の先頭12桁。OCI layoutのarchiveをloadしたruntimeが報告するid。
const INDEX_SHORT_ID: &str = "bbbbbbbbbbbb";

fn built_image() -> BuiltImage {
    BuiltImage {
        name: IMAGE_NAME.to_string(),
        id: MATCHING_ID.to_string(),
        labels: Vec::new(),
        built: false,
        warnings: Vec::new(),
    }
}

/// OCI layoutのarchive。containerd image storeの`docker image save`が書く形。
fn archive_path(dir: &std::path::Path) -> Checked<std::path::PathBuf> {
    let path = dir.join("archive.tar");
    fs::write(&path, image_archive_bytes(IMAGE_NAME, MATCHING_ID, &[]))
        .required_because("write the test archive")?;
    Ok(path)
}

/// legacy layoutのarchive。`index.json`を持たず、image configのdigestだけを宣言する。
fn legacy_archive_path(dir: &std::path::Path) -> Checked<std::path::PathBuf> {
    let path = dir.join("legacy.tar");
    let hex = &MATCHING_ID["sha256:".len()..];
    let blob = format!("blobs/sha256/{hex}");
    let manifest = manifest_json(IMAGE_NAME, MATCHING_ID);
    fs::write(
        &path,
        tar_bytes(&[
            (blob.as_str(), br#"{"config":{"Labels":{}}}"#),
            ("manifest.json", manifest.as_bytes()),
        ]),
    )
    .required_because("write the legacy test archive")?;
    Ok(path)
}

fn listing(id: &str) -> String {
    format!(
        r#"{{"images":[{{"id":"{id}","repository":"docker.io/library/sbxm-example-template","tag":"aaaaaaaaaaaa"}}]}}"#
    )
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
fn a_template_reported_by_the_index_digest_of_the_verified_archive_is_reused() -> Checked {
    // containerd image storeでは、build結果もloadしたTemplateもimage indexのdigestで
    // 識別される。image configのdigestと突き合わせると、同じimageのTemplateを拒む。
    assert_eq!(
        &index_id(MATCHING_ID)["sha256:".len()..][..12],
        INDEX_SHORT_ID
    );
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering("template ls --json", 0, &listing(INDEX_SHORT_ID));
    let loaded = verified_existing(&host, &built_image(), &archive_path(dir.path())?)
        .required_because("the index digest identifies the same image")?;
    assert_eq!(
        loaded.map(|template| template.name),
        Some(IMAGE_NAME.to_string())
    );
    Ok(())
}

#[test]
fn a_template_reported_by_the_config_digest_of_a_legacy_archive_is_reused() -> Checked {
    // indexを持たないarchiveでは、runtimeはimage configのdigestをidにする。
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering("template ls --json", 0, &listing("aaaaaaaaaaaa"));
    let loaded = verified_existing(&host, &built_image(), &legacy_archive_path(dir.path())?)
        .required_because("a matching config digest is reused")?;
    assert_eq!(
        loaded.map(|template| template.name),
        Some(IMAGE_NAME.to_string())
    );
    Ok(())
}

#[test]
fn a_template_reported_by_the_config_digest_of_an_oci_archive_is_still_reused() -> Checked {
    // OCI layoutのarchiveでも、runtimeがconfigのdigestで登録する構成を拒む理由はない。
    // どちらもこのarchiveが宣言するdigestである。
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering("template ls --json", 0, &listing("aaaaaaaaaaaa"));
    let loaded = verified_existing(&host, &built_image(), &archive_path(dir.path())?)
        .required_because("the config digest identifies the same image")?;
    assert!(loaded.is_some());
    Ok(())
}

#[test]
fn a_template_whose_runtime_id_differs_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let host = FakeSbx::listing("").answering("template ls --json", 0, &listing("deadbeefdead"));
    let error = verified_existing(&host, &built_image(), &archive_path(dir.path())?)
        .refused_because("a different runtime id is not the verified image")?;
    assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    // 期待値には、archiveが宣言するdigestがすべて並ぶ。
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(INDEX_SHORT_ID) && rendered.contains("aaaaaaaaaaaa"),
        "{rendered}"
    );
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
