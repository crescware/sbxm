//! `docker image save`が書くarchiveの偽物。

use crate::archive::{manifest_json, tar_bytes};

/// image configとmanifestだけを持つarchive。
pub fn image_archive_bytes(image_name: &str, image_id: &str, labels: &[(&str, &str)]) -> Vec<u8> {
    // 実物と同じく、archiveはimage configをlabelごと持つ。
    let rendered = labels
        .iter()
        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
        .collect::<Vec<_>>()
        .join(",");
    let config = format!(r#"{{"config":{{"Labels":{{{rendered}}}}}}}"#);
    let hex = image_id.strip_prefix("sha256:").unwrap_or(image_id);
    let blob = format!("blobs/sha256/{hex}");
    let manifest = manifest_json(image_name, image_id);
    tar_bytes(&[
        (blob.as_str(), config.as_bytes()),
        ("manifest.json", manifest.as_bytes()),
    ])
}
