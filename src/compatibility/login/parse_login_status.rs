use crate::diagnostics::{Result, unparseable};

/// `sbx login status`からlogin済みかどうかを読む。
///
/// 真偽を示すfieldがない出力から、login済みだと推測しない。
pub fn parse_login_status(output: &str) -> Result<bool> {
    let document: serde_json::Value = serde_json::from_str(output.trim())
        .map_err(|error| unparseable("sbx login status", &error.to_string()))?;
    let object = document
        .as_object()
        .ok_or_else(|| unparseable("sbx login status", "the document is not an object"))?;

    for key in ["logged_in", "loggedIn", "authenticated", "signed_in"] {
        if let Some(value) = object.get(key) {
            return value.as_bool().ok_or_else(|| {
                unparseable("sbx login status", &format!("{key} is not a boolean"))
            });
        }
    }
    Err(unparseable(
        "sbx login status",
        "no field states whether this host is signed in",
    ))
}
