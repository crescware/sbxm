use std::path::Path;

use crate::diagnostics::Result;
use crate::image_labels::{LabelDefect, labels_from_declared};
use crate::msg;

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
            msg!(
                "cause-archive-holds-another-image",
                observed = manifest.repo_tags.join(", "),
                declared = image_name
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
                    msg!(
                        "cause-archive-label-differs",
                        key = key,
                        observed = observed.map_or("<absent>", String::as_str),
                        expected = expected
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
            msg!("cause-archive-entry-absent", entry = config_entry),
        ));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        unusable(
            path,
            msg!(
                "cause-archive-entry-not-json",
                entry = config_entry,
                detail = error
            ),
        )
    })?;

    // image configはOCIとDockerのどちらの表記でも`config`objectの下にlabelを持つ。
    let config = document
        .get("config")
        .or_else(|| document.get("Config"))
        .and_then(|value| value.as_object())
        .ok_or_else(|| {
            unusable(
                path,
                msg!("cause-archive-entry-has-no-config", entry = config_entry),
            )
        })?;

    let declared = config.get("Labels").or_else(|| config.get("labels"));
    labels_from_declared(declared).map_err(|defect| match defect {
        LabelDefect::NotAnObject => unusable(
            path,
            msg!("cause-archive-labels-not-an-object", entry = config_entry),
        ),
        LabelDefect::ValueNotAString(key) => unusable(
            path,
            msg!(
                "cause-archive-label-not-a-string",
                key = key,
                entry = config_entry
            ),
        ),
    })
}
