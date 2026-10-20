use crate::diagnostics::{Result, unparseable};

use crate::boundary::host::protocol::json::string_field;

use super::CustomSecret;

/// `sbx secret ls --json`からcustom secretの宛先と公開のplaceholderだけを読む。
/// serviceやregistryのsecret、および`secret`fieldの値には触れない。
pub fn parse_custom_secrets(output: &str) -> Result<Vec<CustomSecret>> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx secret ls", &error.to_string()))?;
    let items = document
        .get("custom_secrets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| unparseable("sbx secret ls", "custom_secrets is not a list"))?;
    items.iter().map(custom_secret).collect()
}

fn custom_secret(document: &serde_json::Value) -> Result<CustomSecret> {
    let object = document
        .as_object()
        .ok_or_else(|| unparseable("sbx secret ls", "a custom secret is not an object"))?;
    let required = |key: &str| {
        string_field(object, key)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| unparseable("sbx secret ls", &format!("a custom secret has no {key}")))
    };
    // `env`と`targets`は省略できる。存在する場合は型を検証し、不正な値を不在と読まない。
    let env = match object.get("env") {
        None => String::new(),
        Some(serde_json::Value::String(value)) => value.clone(),
        Some(_) => return Err(unparseable("sbx secret ls", "env is not a string")),
    };
    let targets = match object.get("targets") {
        None => Vec::new(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| unparseable("sbx secret ls", "a target is not a string"))
            })
            .collect::<Result<Vec<_>>>()?,
        Some(_) => return Err(unparseable("sbx secret ls", "targets is not a list")),
    };
    Ok(CustomSecret {
        scope: required("scope")?,
        targets,
        env,
        placeholder: required("placeholder")?,
    })
}
