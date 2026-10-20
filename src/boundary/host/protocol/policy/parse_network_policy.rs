use crate::diagnostics::{Result, unparseable};

use super::NetworkPolicyState;

/// v0.48.0のpolicy一覧を読む。preset名はこの出力にはなく、推測しない。
pub fn parse_network_policy(output: &str) -> Result<NetworkPolicyState> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx policy ls", &error.to_string()))?;
    let rules = document
        .get("rules")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| unparseable("sbx policy ls", "rules is not a list"))?;
    if rules.iter().any(|rule| !rule.is_object()) {
        return Err(unparseable("sbx policy ls", "a rule is not an object"));
    }
    let flag = |key: &str| -> Result<bool> {
        match document.get(key) {
            None => Ok(false),
            Some(serde_json::Value::Bool(value)) => Ok(*value),
            Some(_) => Err(unparseable(
                "sbx policy ls",
                &format!("{key} is not a boolean"),
            )),
        }
    };
    let blocked = flag("governance_blocked")?;
    let unavailable = flag("organization_unavailable")?;
    Ok(if blocked || unavailable {
        NetworkPolicyState::Blocked
    } else {
        NetworkPolicyState::Available
    })
}
