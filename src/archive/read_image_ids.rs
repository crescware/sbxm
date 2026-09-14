use std::path::Path;

use crate::diagnostics::Result;
use crate::msg;

use super::{INDEX_ENTRY, normalized_digest, read_entry, read_manifest, unreadable, unusable};

/// archiveがimageの同一性として宣言するdigest。
///
/// runtimeはloadしたTemplateのidを、archiveの書き方に応じて別の値で報告する。
/// OCI layout（containerd image storeの`docker image save`）では`index.json`が指す
/// image indexまたはmanifestのdigestになり、image configのdigestとは一致しない。
/// legacy layoutにはindexが無く、idはimage configのdigestになる。
///
/// どちらで報告されても照合できるよう、archiveが宣言する候補をすべて返す。並びは
/// `index.json`が指すdigest、次にimage configのdigestとする。
pub fn read_image_ids(path: &Path) -> Result<Vec<String>> {
    let mut ids = read_index_digests(path)?;
    let manifest = read_manifest(path)?;
    if !ids.contains(&manifest.config_digest) {
        ids.push(manifest.config_digest);
    }
    Ok(ids)
}

/// `index.json`が列挙するmanifestのdigest。indexを持たないarchiveでは空になる。
fn read_index_digests(path: &Path) -> Result<Vec<String>> {
    let Some(bytes) = read_entry(path, INDEX_ENTRY)? else {
        return Ok(Vec::new());
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unreadable(path, Some(INDEX_ENTRY), &error.to_string()))?;
    let manifests = document
        .get("manifests")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            unusable(
                path,
                msg!(
                    "cause-archive-index-lists-no-manifests",
                    entry = INDEX_ENTRY
                ),
            )
        })?;

    manifests
        .iter()
        .map(|item| {
            let written = item.get("digest").and_then(|value| value.as_str());
            normalized_digest(written.unwrap_or_default()).ok_or_else(|| {
                unusable(
                    path,
                    msg!(
                        "cause-archive-index-digest-unusable",
                        entry = INDEX_ENTRY,
                        observed = written.unwrap_or("<absent>")
                    ),
                )
            })
        })
        .collect()
}
