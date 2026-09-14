use crate::archive::{index_json, manifest_json, tar_bytes};

use super::index_id;

/// image config、manifest、そしてOCI layoutのindexを持つarchive。
///
/// containerd image storeの`docker image save`と同じく、`index.json`が指すdigestは
/// image configのdigestとは別の値になる。runtimeがloadしたTemplateへ報告するidは
/// 前者であり、後者ではない。
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
    let index = index_json(image_name, &index_id(image_id));
    tar_bytes(&[
        (blob.as_str(), config.as_bytes()),
        ("index.json", index.as_bytes()),
        ("manifest.json", manifest.as_bytes()),
    ])
}
