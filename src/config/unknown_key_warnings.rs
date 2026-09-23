use std::path::Path;

use crate::design::Warning;
use crate::msg;
use crate::paths::{self};

use super::{KNOWN_FILE_KEYS, KNOWN_TOP_LEVEL_KEYS};

/// 既知でないtop-level keyと、宣言fileの既知でないkeyを警告として集める。
///
/// 未知のkeyは読み飛ばすが、黙って捨てると設定した側が気づけない。宣言fileには後の
/// versionが項目を足しうる。古いbuildはその項目を解さないまま配置することになるため、
/// 解さなかったことを示す。
pub(super) fn unknown_key_warnings(document: &yaml_serde::Value, path: &Path) -> Vec<Warning> {
    let Some(mapping) = document.as_mapping() else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    for name in unknown_keys(mapping, KNOWN_TOP_LEVEL_KEYS) {
        warnings.push(Warning::text(msg!(
            "warning-config-unknown-key",
            path = paths::display(path),
            key = name
        )));
    }
    let files = mapping
        .get("files")
        .and_then(yaml_serde::Value::as_sequence);
    for (index, entry) in files.into_iter().flatten().enumerate() {
        let Some(entry) = entry.as_mapping() else {
            continue;
        };
        for name in unknown_keys(entry, KNOWN_FILE_KEYS) {
            warnings.push(Warning::text(msg!(
                "warning-config-unknown-file-key",
                path = paths::display(path),
                entry = index,
                key = name
            )));
        }
    }
    warnings
}

/// `known`に無いkey。
fn unknown_keys(mapping: &yaml_serde::Mapping, known: &[&str]) -> Vec<String> {
    mapping
        .keys()
        // YAMLのkeyは文字列とは限らない。既知keyはすべて文字列なので、
        // 文字列でないkeyはその表記のまま未知として報告する。
        .map(|key| {
            key.as_str()
                .map_or_else(|| format!("{key:?}"), str::to_string)
        })
        .filter(|name| !known.contains(&name.as_str()))
        .collect()
}
