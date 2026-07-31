use crate::diagnostics::Result;

use super::unparseable;

/// 一覧形式と1行1件のJSON形式のどちらでも読む。
pub fn json_documents(program: &str, output: &str) -> Result<Vec<serde_json::Value>> {
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
