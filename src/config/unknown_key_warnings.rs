use std::path::Path;

use crate::design::Warning;
use crate::msg;
use crate::paths::{self};

use super::KNOWN_TOP_LEVEL_KEYS;

/// 既知でないtop-level keyを警告として集める。
///
/// 未知のkeyは読み飛ばすが、黙って捨てると設定した側が気づけない。
pub(super) fn unknown_key_warnings(document: &yaml_serde::Value, path: &Path) -> Vec<Warning> {
    let Some(mapping) = document.as_mapping() else {
        return Vec::new();
    };
    let mut warnings = Vec::new();
    for key in mapping.keys() {
        // YAMLのkeyは文字列とは限らない。既知keyはすべて文字列なので、
        // 文字列でないkeyはその表記のまま未知として報告する。
        let name = key
            .as_str()
            .map_or_else(|| format!("{key:?}"), str::to_string);
        if !KNOWN_TOP_LEVEL_KEYS.contains(&name.as_str()) {
            warnings.push(Warning::text(msg!(
                "warning-config-unknown-key",
                path = paths::display(path),
                key = name
            )));
        }
    }
    warnings
}
