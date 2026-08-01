use std::path::Path;

use crate::diagnostics::Result;
use crate::msg;

use super::{ArchiveManifest, MANIFEST_ENTRY, config_digest, read_entry, unreadable, unusable};

/// archiveのmanifestを読む。
pub fn read_manifest(path: &Path) -> Result<ArchiveManifest> {
    let Some(bytes) = read_entry(path, MANIFEST_ENTRY)? else {
        return Err(unusable(
            path,
            msg!("cause-archive-entry-absent", entry = MANIFEST_ENTRY),
        ));
    };
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| unreadable(path, Some(MANIFEST_ENTRY), &error.to_string()))?;
    let items = document.as_array().ok_or_else(|| {
        unusable(
            path,
            msg!("cause-archive-entry-not-an-array", entry = MANIFEST_ENTRY),
        )
    })?;
    let [item] = items.as_slice() else {
        return Err(unusable(
            path,
            msg!("cause-archive-holds-several-images", count = items.len()),
        ));
    };

    let config = item
        .get("Config")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            unusable(
                path,
                msg!("cause-archive-names-no-config", entry = MANIFEST_ENTRY),
            )
        })?;
    let digest = config_digest(config).ok_or_else(|| {
        unusable(
            path,
            msg!("cause-archive-config-not-a-digest", observed = config),
        )
    })?;

    let repo_tags = match item.get("RepoTags") {
        Some(serde_json::Value::Array(tags)) => tags
            .iter()
            .map(|tag| {
                tag.as_str()
                    .map(std::string::ToString::to_string)
                    .ok_or_else(|| unusable(path, msg!("cause-archive-tag-not-a-string")))
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
