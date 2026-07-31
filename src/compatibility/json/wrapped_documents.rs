use crate::diagnostics::Result;

use super::{json_documents, unparseable};

/// 一覧を`key`で包んだ出力を読む。
///
/// 包みのない形も、行区切りの形も受け付ける。包みの値が`null`なら0件とする。
pub fn wrapped_documents(program: &str, key: &str, output: &str) -> Result<Vec<serde_json::Value>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(serde_json::Value::Object(object)) =
        serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(listed) = object.get(key)
    {
        return match listed {
            serde_json::Value::Array(items) => Ok(items.clone()),
            serde_json::Value::Null => Ok(Vec::new()),
            _ => Err(unparseable(program, &format!("{key} is not a list"))),
        };
    }
    json_documents(program, output)
}
