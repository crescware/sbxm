use std::path::Path;

use crate::diagnostics::Result;

use super::{ArchiveManifest, MANIFEST_ENTRY, config_digest, read_entry, unusable};

/// archiveのmanifestを読む。
pub fn read_manifest(path: &Path) -> Result<ArchiveManifest> {
    let Some(bytes) = read_entry(path, MANIFEST_ENTRY)? else {
        return Err(unusable(
            path,
            &format!("the archive has no {MANIFEST_ENTRY}"),
        ));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unusable(path, &format!("{MANIFEST_ENTRY} is not JSON: {error}")))?;
    let items = document
        .as_array()
        .ok_or_else(|| unusable(path, &format!("{MANIFEST_ENTRY} is not an array")))?;
    let [item] = items.as_slice() else {
        return Err(unusable(
            path,
            &format!("the archive holds {} images instead of one", items.len()),
        ));
    };

    let config = item
        .get("Config")
        .and_then(|value| value.as_str())
        .ok_or_else(|| unusable(path, &format!("{MANIFEST_ENTRY} names no image config")))?;
    let digest = config_digest(config)
        .ok_or_else(|| unusable(path, &format!("{config} is not an image config digest")))?;

    let repo_tags = match item.get("RepoTags") {
        Some(serde_json::Value::Array(tags)) => tags
            .iter()
            .map(|tag| {
                tag.as_str()
                    .map(std::string::ToString::to_string)
                    .ok_or_else(|| unusable(path, "a repository tag is not a string"))
            })
            .collect::<Result<Vec<String>>>()?,
        // tagを持たないarchiveからは、どのimageを保存したかを判定できない。
        _ => Vec::new(),
    };

    Ok(ArchiveManifest {
        repo_tags,
        config_digest: digest,
        config_entry: config.to_string(),
    })
}
