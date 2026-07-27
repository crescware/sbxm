//! Docker Sandboxes CLIの出力を解釈する。
//!
//! 解釈できない出力から状態を推測しない。parseできない出力はerrorとして扱う。

use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// 要件となる最小version。
pub const MINIMUM_CLI_VERSION: CliVersion = CliVersion {
    major: 0,
    minor: 37,
    patch: 0,
};

/// 期待するnetwork policy。ほかのpolicyは、より制限が強い場合も含めて対応しない。
pub const EXPECTED_NETWORK_POLICY: &str = "Balanced";

/// `<major>.<minor>.<patch>`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CliVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl CliVersion {
    /// `0.37.0`のような厳密な3要素表記だけを受け付ける。
    pub fn parse(value: &str) -> Option<CliVersion> {
        let mut parts = value.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(CliVersion {
            major,
            minor,
            patch,
        })
    }

    /// `sbx version`の出力から最初のversion表記を取り出す。
    ///
    /// 数値3要素の並びだけを認め、見つからない場合はparse不能として扱う。
    pub fn extract_from_output(output: &str) -> Option<CliVersion> {
        for token in output.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
            if let Some(version) = CliVersion::parse(token) {
                return Some(version);
            }
        }
        None
    }
}

impl std::fmt::Display for CliVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 検出したversionが最小要件を満たすかを判定する。
pub fn require_minimum_version(observed: CliVersion) -> Result<()> {
    if observed >= MINIMUM_CLI_VERSION {
        return Ok(());
    }
    Err(Error::new(
        ErrorId::SbxVersionBelowMinimum,
        msg!(
            "error-sbx-version-below-minimum",
            observed = observed,
            minimum = MINIMUM_CLI_VERSION
        ),
    ))
}

/// daemonの状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Running,
    Stopped,
}

/// `sbx daemon status`をparseする。
pub fn parse_daemon_status(output: &str) -> Result<DaemonState> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx daemon status", &error.to_string()))?;
    let object = document
        .as_object()
        .ok_or_else(|| unparseable("sbx daemon status", "the document is not an object"))?;

    if let Some(running) = object.get("running").and_then(|value| value.as_bool()) {
        return Ok(if running {
            DaemonState::Running
        } else {
            DaemonState::Stopped
        });
    }
    match string_field(object, "state")
        .or_else(|| string_field(object, "status"))
        .as_deref()
    {
        Some("running") => Ok(DaemonState::Running),
        Some("stopped") | Some("not-running") => Ok(DaemonState::Stopped),
        Some(other) => Err(unparseable(
            "sbx daemon status",
            &format!("state {other} has no defined meaning in this build"),
        )),
        None => Err(unparseable(
            "sbx daemon status",
            "the state field is absent",
        )),
    }
}

/// `sbx policy ls`から現在のnetwork policyを取り出す。
pub fn parse_network_policy(output: &str) -> Result<String> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx policy ls", &error.to_string()))?;

    let from_object = |object: &serde_json::Map<String, serde_json::Value>| -> Option<String> {
        string_field(object, "policy")
            .or_else(|| string_field(object, "current"))
            .or_else(|| string_field(object, "name"))
    };

    match &document {
        serde_json::Value::Object(object) => from_object(object)
            .ok_or_else(|| unparseable("sbx policy ls", "no policy field is present")),
        serde_json::Value::Array(items) => {
            // 一覧形式では、有効と印の付いた1件だけを現在値とする。
            let mut active: Vec<String> = Vec::new();
            for item in items {
                let Some(object) = item.as_object() else {
                    return Err(unparseable("sbx policy ls", "an entry is not an object"));
                };
                let selected = object
                    .get("active")
                    .or_else(|| object.get("current"))
                    .or_else(|| object.get("selected"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if selected && let Some(name) = from_object(object) {
                    active.push(name);
                }
            }
            match active.len() {
                1 => Ok(active.remove(0)),
                0 => Err(unparseable(
                    "sbx policy ls",
                    "no entry is marked as the active policy",
                )),
                _ => Err(unparseable(
                    "sbx policy ls",
                    "more than one entry is marked as the active policy",
                )),
            }
        }
        _ => Err(unparseable(
            "sbx policy ls",
            "the document is neither an array nor an object",
        )),
    }
}

fn string_field(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn unparseable(program: &str, detail: &str) -> Error {
    Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = program,
            detail = detail
        ),
    )
}

/// 未対応のcommandを一時的に拒否する共通error。
pub fn not_implemented(command: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::NotImplemented,
            msg!("error-not-implemented", command = command),
        )
        .remediation(msg!("remediation-run-help", command = "sbxm --help")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_require_exactly_three_numeric_parts() {
        assert_eq!(
            CliVersion::parse("0.37.0"),
            Some(CliVersion {
                major: 0,
                minor: 37,
                patch: 0
            })
        );
        assert_eq!(CliVersion::parse("0.37"), None);
        assert_eq!(CliVersion::parse("0.37.0.1"), None);
        assert_eq!(CliVersion::parse("0.37.x"), None);
        assert_eq!(CliVersion::parse(""), None);
    }

    #[test]
    fn versions_are_extracted_from_surrounding_text() {
        assert_eq!(
            CliVersion::extract_from_output("sbx version 0.37.2\n"),
            CliVersion::parse("0.37.2")
        );
        assert_eq!(
            CliVersion::extract_from_output("Docker Sandboxes CLI v1.2.3 (build abc)"),
            CliVersion::parse("1.2.3")
        );
        assert_eq!(CliVersion::extract_from_output("no version here"), None);
        assert_eq!(CliVersion::extract_from_output(""), None);
    }

    #[test]
    fn versions_below_the_minimum_are_refused() {
        let error = require_minimum_version(CliVersion::parse("0.36.9").unwrap())
            .expect_err("an older version must be refused");
        assert_eq!(error.first_id(), Some(ErrorId::SbxVersionBelowMinimum));
    }

    #[test]
    fn the_minimum_version_and_later_are_accepted() {
        for observed in ["0.37.0", "0.37.5", "0.38.0", "1.0.0"] {
            assert!(
                require_minimum_version(CliVersion::parse(observed).unwrap()).is_ok(),
                "{observed} must be accepted"
            );
        }
    }

    #[test]
    fn the_daemon_status_parser_maps_only_known_states() {
        assert_eq!(
            parse_daemon_status(r#"{"running": true}"#).unwrap(),
            DaemonState::Running
        );
        assert_eq!(
            parse_daemon_status(r#"{"running": false}"#).unwrap(),
            DaemonState::Stopped
        );
        assert_eq!(
            parse_daemon_status(r#"{"state": "running"}"#).unwrap(),
            DaemonState::Running
        );

        for output in ["{}", r#"{"state":"degraded"}"#, "[]", "oops"] {
            let error = parse_daemon_status(output).expect_err("unknown states are not guessed");
            assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        }
    }

    #[test]
    fn the_network_policy_parser_reads_the_active_entry_only() {
        let balanced = r#"[{"name":"Balanced","active":true},{"name":"Open","active":false}]"#;
        assert_eq!(parse_network_policy(balanced).unwrap(), "Balanced");

        let other = r#"[{"name":"Balanced","active":false},{"name":"Open","active":true}]"#;
        assert_ne!(
            parse_network_policy(other).unwrap(),
            EXPECTED_NETWORK_POLICY
        );

        for output in [
            "{}",
            r#"[{"name":"Balanced","active":false}]"#,
            r#"[{"name":"Balanced","active":true},{"name":"Open","active":true}]"#,
        ] {
            let error =
                parse_network_policy(output).expect_err("an ambiguous policy is not guessed");
            assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        }
    }
}
