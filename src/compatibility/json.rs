//! structured outputの共通部分。
//!
//! 解釈できない出力から状態を推測せず、同じ形のerrorで報告する。

use crate::error::{Error, ErrorId, Result};
use crate::msg;

/// 一覧を`key`で包んだ出力を読む。
///
/// 包みのない形も、行区切りの形も受け付ける。包みの値が`null`なら0件とする。
pub(super) fn wrapped_documents(
    program: &str,
    key: &str,
    output: &str,
) -> Result<Vec<serde_json::Value>> {
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

/// 一覧形式と1行1件のJSON形式のどちらでも読む。
pub(super) fn json_documents(program: &str, output: &str) -> Result<Vec<serde_json::Value>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        // 1件もないことは観測できた状態であり、推測ではない。
        return Ok(Vec::new());
    }
    match serde_json::from_str(trimmed) {
        Ok(serde_json::Value::Array(items)) => Ok(items),
        Ok(serde_json::Value::Object(object)) => Ok(vec![serde_json::Value::Object(object)]),
        Ok(_) => Err(unparseable(
            program,
            "the document is neither an array nor an object",
        )),
        Err(_) => trimmed
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line).map_err(|error| unparseable(program, &error.to_string()))
            })
            .collect(),
    }
}

pub(super) fn string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .map(std::string::ToString::to_string)
}

pub(super) fn unparseable(program: &str, detail: &str) -> Error {
    Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = program,
            detail = detail
        ),
    )
}
