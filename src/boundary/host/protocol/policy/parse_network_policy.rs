use crate::diagnostics::{Result, unparseable};

use super::EXPECTED_NETWORK_POLICY;

/// v0.48.0の組み込みBalanced presetを構成するnetwork ruleのID。
const BALANCED_RULES: [&str; 6] = [
    "default-ai-services",
    "default-package-managers",
    "default-code-and-containers",
    "default-cloud-infrastructure",
    "default-os-packages",
    "default-cert-validation",
];

/// 組み込みpresetのrule IDと出自を確認する。任意のallow/deny ruleからpresetを推測しない。
pub fn parse_network_policy(output: &str) -> Result<String> {
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
    if blocked || unavailable {
        return Err(unparseable(
            "sbx policy ls",
            "governance is blocked or unavailable",
        ));
    }
    let defaults: Vec<&serde_json::Value> = rules
        .iter()
        .filter(|rule| {
            rule.get("origin").and_then(serde_json::Value::as_str) == Some("local")
                && rule
                    .pointer("/provenance/created_via")
                    .and_then(serde_json::Value::as_str)
                    == Some("default")
                && rule.get("sandbox_id").is_none()
                && rule.get("status").and_then(serde_json::Value::as_str) == Some("active")
        })
        .collect();
    let network: Vec<&serde_json::Value> = defaults
        .iter()
        .copied()
        .filter(|rule| {
            rule.get("resource_type")
                .and_then(serde_json::Value::as_str)
                == Some("network")
        })
        .collect();
    if network
        .iter()
        .any(|rule| rule.get("id").and_then(serde_json::Value::as_str) == Some("default-allow-all"))
    {
        return Ok("Open".to_string());
    }
    if network.len() == BALANCED_RULES.len()
        && BALANCED_RULES.iter().all(|id| {
            network
                .iter()
                .filter(|rule| {
                    rule.get("id").and_then(serde_json::Value::as_str) == Some(*id)
                        && rule.get("decision").and_then(serde_json::Value::as_str) == Some("allow")
                        && rule
                            .get("resources")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|values| {
                                !values.is_empty()
                                    && values.iter().all(serde_json::Value::is_string)
                            })
                })
                .count()
                == 1
        })
    {
        return Ok(EXPECTED_NETWORK_POLICY.to_string());
    }
    if network.is_empty()
        && ["default-fs-read-allow-all", "default-fs-write-allow-all"]
            .iter()
            .all(|id| {
                defaults
                    .iter()
                    .any(|rule| rule.get("id").and_then(serde_json::Value::as_str) == Some(*id))
            })
    {
        return Ok("Locked Down".to_string());
    }
    Err(unparseable(
        "sbx policy ls",
        "the active built-in network preset cannot be established",
    ))
}
