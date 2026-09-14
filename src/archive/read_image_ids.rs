use std::path::Path;

use crate::diagnostics::Result;
use crate::msg;

use super::{INDEX_ENTRY, normalized_digest, read_entry, read_manifest, unreadable, unusable};

/// archiveがimageの同一性として宣言するdigest。
///
/// runtimeはloadしたTemplateのidを、archiveの書き方とruntime自身のimage storeに
/// 応じて別の値で報告する。containerd image storeを持つruntimeは、OCI layout
/// （containerd storeの`docker image save`）なら`index.json`が指すimage indexまたは
/// manifestのdigestを報告し、image configのdigestとは一致しない。graph driverの
/// storeを持つruntimeは、どちらのlayoutでもimage configのdigestを報告する。
///
/// archiveが宣言する候補をすべて返す。並びは`index.json`が指すdigest、次にimage
/// configのdigestとする。legacy layout（indexを持たない）をcontainerd storeの
/// runtimeへloadした場合、idはruntimeが合成したmanifestのdigestになり、archiveの
/// どの値からも導けない。その組み合わせは照合できず、呼び出し側で再利用を拒む。
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
        .filter(|manifests| !manifests.is_empty())
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
