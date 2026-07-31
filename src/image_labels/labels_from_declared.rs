use std::collections::BTreeMap;

use super::LabelDefect;

/// 宣言された値をlabel集合として読む。
///
/// labelを1つも持たないimageでは`null`になる。
pub fn labels_from_declared(
    declared: Option<&serde_json::Value>,
) -> std::result::Result<BTreeMap<String, String>, LabelDefect> {
    let mut labels = BTreeMap::new();
    match declared {
        Some(serde_json::Value::Object(entries)) => {
            for (key, value) in entries {
                let value = value
                    .as_str()
                    .ok_or_else(|| LabelDefect::ValueNotAString(key.clone()))?;
                labels.insert(key.clone(), value.to_string());
            }
        }
        Some(serde_json::Value::Null) | None => {}
        Some(_) => return Err(LabelDefect::NotAnObject),
    }
    Ok(labels)
}
