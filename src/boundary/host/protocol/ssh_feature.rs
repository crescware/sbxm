use crate::diagnostics::{Result, unparseable};

/// `sbx settings get feature.ssh --json`の実効値と設定元。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshFeature {
    pub enabled: bool,
    pub source: String,
    pub locked: bool,
    pub requires_restart: bool,
}

impl SshFeature {
    pub fn parse(output: &str) -> Result<Self> {
        let document: serde_json::Value = serde_json::from_str(output)
            .map_err(|error| unparseable("sbx settings get feature.ssh", &error.to_string()))?;
        let invalid = || {
            unparseable(
                "sbx settings get feature.ssh",
                "expected the effective feature.ssh setting, its enabled value and source",
            )
        };
        if document.get("key").and_then(serde_json::Value::as_str) != Some("feature.ssh") {
            return Err(invalid());
        }
        let enabled = document
            .get("value")
            .and_then(|value| value.get("enabled"))
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(invalid)?;
        let source = document
            .get("source")
            .and_then(serde_json::Value::as_str)
            .filter(|source| !source.is_empty())
            .ok_or_else(invalid)?
            .to_string();
        let locked = match document.get("locked") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(invalid)?,
        };
        let requires_restart = document
            .get("requires_restart")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(invalid)?;
        Ok(Self {
            enabled,
            source,
            locked,
            requires_restart,
        })
    }

    pub fn can_enable(&self) -> bool {
        !self.locked && matches!(self.source.as_str(), "default" | "override")
    }

    /// SSHをgateする管理者側の設定。値を推測せず、boolとして読めた場合だけ返す。
    pub fn parse_experimental(output: &str) -> Result<bool> {
        let document: serde_json::Value = serde_json::from_str(output).map_err(|error| {
            unparseable(
                "sbx settings get platform.allowExperimentalFeatures",
                &error.to_string(),
            )
        })?;
        if document.get("key").and_then(serde_json::Value::as_str)
            == Some("platform.allowExperimentalFeatures")
            && let Some(value) = document.get("value").and_then(serde_json::Value::as_bool)
        {
            return Ok(value);
        }
        Err(unparseable(
            "sbx settings get platform.allowExperimentalFeatures",
            "expected the effective experimental-features boolean",
        ))
    }
}
