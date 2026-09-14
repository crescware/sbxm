use crate::diagnostics::{Result, unparseable};

use crate::boundary::host::protocol::json::string_field;

use super::{CustomSecret, SecretListing, ServiceSecret};

const PROGRAM: &str = "sbx secret ls";

/// `sbx secret ls --json`のstructured outputをparseする。
///
/// 受け付ける形はsbx v0.42.1（要件となる最小version）が実際に返すものに絞る。
/// `secrets`はservice secretの一覧で、各entryが`scope`、`type`、`name`を持つ。
/// `custom_secrets`はcustom secretの一覧で、各entryが`scope`、`targets`、`env`、
/// `placeholder`を持つ。どちらも`secret`列は読まない。値の一部が現れるためである。
pub fn parse_secret_listing(output: &str) -> Result<SecretListing> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(unparseable(PROGRAM, "the output is empty"));
    }
    let document: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|error| unparseable(PROGRAM, &format!("the output is not JSON: {error}")))?;
    let object = document
        .as_object()
        .ok_or_else(|| unparseable(PROGRAM, "the output is not an object"))?;

    let mut listing = SecretListing::default();
    for item in entries_of(object, "secrets")? {
        let entry = item
            .as_object()
            .ok_or_else(|| unparseable(PROGRAM, "a secret is not an object"))?;
        // registry credentialなど、serviceでない登録も同じ一覧に並びうる。
        if string_field(entry, "type").as_deref() != Some("service") {
            continue;
        }
        listing.services.push(ServiceSecret {
            scope: required(entry, "scope", "a service secret")?,
            name: required(entry, "name", "a service secret")?,
        });
    }
    for item in entries_of(object, "custom_secrets")? {
        let entry = item
            .as_object()
            .ok_or_else(|| unparseable(PROGRAM, "a custom secret is not an object"))?;
        let targets = match entry.get("targets") {
            Some(serde_json::Value::Array(targets)) => targets
                .iter()
                .map(|target| {
                    target.as_str().map(str::to_string).ok_or_else(|| {
                        unparseable(PROGRAM, "a custom secret target is not a string")
                    })
                })
                .collect::<Result<Vec<String>>>()?,
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(_) => {
                return Err(unparseable(
                    PROGRAM,
                    "a custom secret's targets is not a list",
                ));
            }
        };
        listing.customs.push(CustomSecret {
            scope: required(entry, "scope", "a custom secret")?,
            targets,
            env: required(entry, "env", "a custom secret")?,
            placeholder: required(entry, "placeholder", "a custom secret")?,
        });
    }
    Ok(listing)
}

/// 一覧の鍵が指す配列。鍵が無いか`null`なら0件とする。
fn entries_of<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a [serde_json::Value]> {
    match object.get(key) {
        Some(serde_json::Value::Array(items)) => Ok(items),
        None | Some(serde_json::Value::Null) => Ok(&[]),
        Some(_) => Err(unparseable(PROGRAM, &format!("{key} is not a list"))),
    }
}

fn required(
    entry: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    what: &str,
) -> Result<String> {
    string_field(entry, key)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| unparseable(PROGRAM, &format!("{what} has no {key}")))
}
