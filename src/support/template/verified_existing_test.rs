use crate::diagnostics::ErrorId;
use crate::support::image::BuiltImage;
use crate::testing::host::FakeSbx;
use crate::testing::image::template_listing_with_id;
use crate::testing::outcome::{Checked, Refused, Required};

use super::verified_existing;

const IMAGE_NAME: &str = "sbxm-example-template:aaaaaaaaaaaa";
/// archiveの`index.json`が指すdigest。OCI layoutのarchiveをloadしたruntimeが報告するid。
const INDEX_ID: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
/// image configのdigest。tagと同じ綴りにしないことで、診断文への出現を区別できる。
const CONFIG_ID: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn built_image() -> BuiltImage {
    BuiltImage {
        name: IMAGE_NAME.to_string(),
        id: INDEX_ID.to_string(),
        labels: Vec::new(),
        built: false,
        warnings: Vec::new(),
    }
}

/// OCI layoutのarchiveが宣言するdigest。indexが先、configが後。
fn oci_declared() -> Vec<String> {
    vec![INDEX_ID.to_string(), CONFIG_ID.to_string()]
}

/// legacy layoutのarchiveが宣言するdigest。configだけ。
fn legacy_declared() -> Vec<String> {
    vec![CONFIG_ID.to_string()]
}

#[test]
fn a_template_with_no_matching_name_is_not_reused() -> Checked {
    let host = FakeSbx::listing("").answering("template ls --json", 0, r#"{"images":[]}"#);
    let loaded = verified_existing(&host, &built_image(), &oci_declared())
        .required_because("an absent template is observed, not an error")?;
    assert!(loaded.is_none());
    Ok(())
}

#[test]
fn a_template_reported_by_the_index_digest_of_the_verified_archive_is_reused() -> Checked {
    // containerd image storeでは、build結果もloadしたTemplateもimage indexのdigestで
    // 識別される。image configのdigestと突き合わせると、同じimageのTemplateを拒む。
    let host = FakeSbx::listing("").answering(
        "template ls --json",
        0,
        &template_listing_with_id(IMAGE_NAME, "bbbbbbbbbbbb")?,
    );
    let loaded = verified_existing(&host, &built_image(), &oci_declared())
        .required_because("the index digest identifies the same image")?;
    assert_eq!(
        loaded.map(|template| template.name),
        Some(IMAGE_NAME.to_string())
    );
    Ok(())
}

#[test]
fn a_template_reported_by_the_config_digest_is_reused_when_the_archive_declares_it() -> Checked {
    // graph driverのimage storeを持つruntimeは、image configのdigestをidにする。
    // legacy layoutでもOCI layoutでも、configのdigestはarchiveが宣言する値である。
    for declared in [legacy_declared(), oci_declared()] {
        let host = FakeSbx::listing("").answering(
            "template ls --json",
            0,
            &template_listing_with_id(IMAGE_NAME, "cccccccccccc")?,
        );
        let loaded = verified_existing(&host, &built_image(), &declared)
            .required_because("the config digest identifies the same image")?;
        assert!(loaded.is_some());
    }
    Ok(())
}

#[test]
fn a_template_whose_runtime_id_differs_is_refused() -> Checked {
    let host = FakeSbx::listing("").answering(
        "template ls --json",
        0,
        &template_listing_with_id(IMAGE_NAME, "deadbeefdead")?,
    );
    let error = verified_existing(&host, &built_image(), &oci_declared())
        .refused_because("a different runtime id is not the verified image")?;
    assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    // 期待値には、archiveが宣言するdigestがすべて、宣言された順に並ぶ。
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(r#"("expected", "bbbbbbbbbbbb, cccccccccccc")"#),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn a_template_with_no_reported_id_is_refused_as_unobservable() -> Checked {
    let host = FakeSbx::listing("").answering(
        "template ls --json",
        0,
        r#"{"images":[{"repository":"docker.io/library/sbxm-example-template","tag":"aaaaaaaaaaaa"}]}"#,
    );
    let error = verified_existing(&host, &built_image(), &oci_declared())
        .refused_because("no id means the correspondence cannot be confirmed")?;
    assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));
    Ok(())
}
