use std::path::Path;

use crate::diagnostics::Result;
use crate::image_labels::{LabelDefect, labels_from_declared};

use super::{read_entry, read_manifest, unusable};

/// archiveが、指定したimageを1件だけ含むことを確認する。
///
/// 判定はarchiveが宣言するtagと、image configが持つlabelで行う。digestは
/// image storeとattestationの有無で意味が変わるため、対応の根拠にしない。
pub fn verify_holds_image(
    path: &Path,
    image_name: &str,
    expected_labels: &[(String, String)],
) -> Result<()> {
    let manifest = read_manifest(path)?;

    if !manifest.repo_tags.iter().any(|tag| tag == image_name) {
        return Err(unusable(
            path,
            &format!(
                "the archive holds {}, not {image_name}",
                manifest.repo_tags.join(", ")
            ),
        ));
    }

    let labels = read_config_labels(path, &manifest.config_entry)?;
    for (key, expected) in expected_labels {
        match labels.get(key) {
            Some(observed) if observed == expected => {}
            observed => {
                return Err(unusable(
                    path,
                    &format!(
                        "the image in the archive declares {key}: {}, expected {expected}",
                        observed.map_or("<absent>", String::as_str)
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// archiveへ保存されたimage configのlabel。
///
/// configはmanifestが名前で指すentryであり、archive本体のlayerは読まない。
fn read_config_labels(
    path: &Path,
    config_entry: &str,
) -> Result<std::collections::BTreeMap<String, String>> {
    let Some(bytes) = read_entry(path, config_entry)? else {
        return Err(unusable(
            path,
            &format!("the archive has no {config_entry}"),
        ));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unusable(path, &format!("{config_entry} is not JSON: {error}")))?;

    // image configはOCIとDockerのどちらの表記でも`config`objectの下にlabelを持つ。
    let config = document
        .get("config")
        .or_else(|| document.get("Config"))
        .and_then(|value| value.as_object())
        .ok_or_else(|| unusable(path, &format!("{config_entry} has no image configuration")))?;

    let declared = config.get("Labels").or_else(|| config.get("labels"));
    labels_from_declared(declared).map_err(|defect| match defect {
        LabelDefect::NotAnObject => unusable(
            path,
            &format!("{config_entry} declares labels that are not an object"),
        ),
        LabelDefect::ValueNotAString(key) => unusable(
            path,
            &format!("label {key} in {config_entry} is not a string"),
        ),
    })
}
