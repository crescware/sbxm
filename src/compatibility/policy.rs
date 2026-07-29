//! `sbx policy ls`の解釈。

use crate::error::Result;

use super::json::{string_field, unparseable};

/// 期待するnetwork policy。ほかのpolicyは、より制限が強い場合も含めて対応しない。
pub const EXPECTED_NETWORK_POLICY: &str = "Balanced";

/// `sbx policy ls`から現在のnetwork policyを取り出す。
pub fn parse_network_policy(output: &str) -> Result<String> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx policy ls", &error.to_string()))?;

    let from_object = |object: &serde_json::Map<String, serde_json::Value>| -> Option<String> {
        string_field(object, "policy")
            .or_else(|| string_field(object, "current"))
            .or_else(|| string_field(object, "name"))
    };

    match &document {
        serde_json::Value::Object(object) => from_object(object)
            .ok_or_else(|| unparseable("sbx policy ls", "no policy field is present")),
        serde_json::Value::Array(items) => {
            // 一覧形式では、有効と印の付いた1件だけを現在値とする。
            let mut active: Vec<String> = Vec::new();
            for item in items {
                let Some(object) = item.as_object() else {
                    return Err(unparseable("sbx policy ls", "an entry is not an object"));
                };
                let selected = object
                    .get("active")
                    .or_else(|| object.get("current"))
                    .or_else(|| object.get("selected"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if selected && let Some(name) = from_object(object) {
                    active.push(name);
                }
            }
            match active.len() {
                1 => Ok(active.remove(0)),
                0 => Err(unparseable(
                    "sbx policy ls",
                    "no entry is marked as the active policy",
                )),
                _ => Err(unparseable(
                    "sbx policy ls",
                    "more than one entry is marked as the active policy",
                )),
            }
        }
        _ => Err(unparseable(
            "sbx policy ls",
            "the document is neither an array nor an object",
        )),
    }
}

#[cfg(test)]
#[path = "policy_test.rs"]
mod policy_test;
