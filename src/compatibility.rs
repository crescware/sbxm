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

/// `docker image inspect`から読むimageの同一性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageIdentity {
    /// `sha256:<hex>`。archiveのconfig blobと同じ値になる。
    pub id: String,
    pub labels: std::collections::BTreeMap<String, String>,
}

/// `docker image inspect <image>`のstructured outputをparseする。
///
/// 1件のimageを指すため、要素が1個の配列だけを受け付ける。labelを持たないimageは
/// 空のlabel集合として扱い、labelの不足は呼び出し側が判定する。
pub fn parse_image_inspect(output: &str) -> Result<ImageIdentity> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("docker image inspect", &error.to_string()))?;
    let items = document
        .as_array()
        .ok_or_else(|| unparseable("docker image inspect", "the document is not an array"))?;
    let [item] = items.as_slice() else {
        return Err(unparseable(
            "docker image inspect",
            &format!(
                "the document describes {} images instead of one",
                items.len()
            ),
        ));
    };
    let object = item
        .as_object()
        .ok_or_else(|| unparseable("docker image inspect", "the entry is not an object"))?;

    let id = string_field(object, "Id")
        .filter(|id| !id.is_empty())
        .ok_or_else(|| unparseable("docker image inspect", "the image has no Id"))?;

    let mut labels = std::collections::BTreeMap::new();
    match object.get("Config").and_then(|config| config.as_object()) {
        Some(config) => match config.get("Labels") {
            Some(serde_json::Value::Object(declared)) => {
                for (key, value) in declared {
                    let value = value.as_str().ok_or_else(|| {
                        unparseable(
                            "docker image inspect",
                            &format!("label {key} does not hold a string"),
                        )
                    })?;
                    labels.insert(key.clone(), value.to_string());
                }
            }
            // labelを1つも持たないimageでは`null`になる。
            Some(serde_json::Value::Null) | None => {}
            Some(_) => {
                return Err(unparseable(
                    "docker image inspect",
                    "Labels is neither an object nor null",
                ));
            }
        },
        None => {
            return Err(unparseable(
                "docker image inspect",
                "the image has no Config section",
            ));
        }
    }

    Ok(ImageIdentity { id, labels })
}

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

/// Sandboxのruntime状態。未知の値を既知の値へ丸めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxState {
    Running,
    Stopped,
}

/// `sbx ls`が示すSandbox 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEntry {
    pub name: String,
    pub state: SandboxState,
    /// Sandboxへ渡したworkspace。示されない場合は`None`。
    pub workspace: Option<String>,
    /// 元にしたTemplate。示されない場合は`None`。
    pub template: Option<String>,
    /// 接続中のsession数。runtimeが示さない場合は`None`。
    pub active_sessions: Option<u64>,
}

/// `sbx ls --json`のstructured outputをparseする。
pub fn parse_sandbox_list(output: &str) -> Result<Vec<SandboxEntry>> {
    let documents = json_documents("sbx ls", output)?;

    let mut entries = Vec::with_capacity(documents.len());
    for document in documents {
        let object = document
            .as_object()
            .ok_or_else(|| unparseable("sbx ls", "an entry is not an object"))?;

        let name = string_field(object, "name")
            .or_else(|| string_field(object, "Name"))
            .filter(|name| !name.is_empty())
            .ok_or_else(|| unparseable("sbx ls", "an entry has no name"))?;

        let observed = string_field(object, "state")
            .or_else(|| string_field(object, "status"))
            .or_else(|| string_field(object, "State"))
            .or_else(|| string_field(object, "Status"))
            .ok_or_else(|| unparseable("sbx ls", &format!("sandbox {name} has no state")))?;
        let state = match observed.to_ascii_lowercase().as_str() {
            "running" => SandboxState::Running,
            "stopped" => SandboxState::Stopped,
            other => {
                return Err(unparseable(
                    "sbx ls",
                    &format!("state {other} has no defined meaning in this build"),
                ));
            }
        };

        let workspace = string_field(object, "workspace")
            .or_else(|| string_field(object, "Workspace"))
            .filter(|value| !value.is_empty());
        let template = string_field(object, "template")
            .or_else(|| string_field(object, "Template"))
            .filter(|value| !value.is_empty());
        let active_sessions = active_sessions(object)?;

        entries.push(SandboxEntry {
            name,
            state,
            workspace,
            template,
            active_sessions,
        });
    }
    Ok(entries)
}

/// 接続中のsession数。数でも一覧でも読めるが、示されない場合は推測しない。
fn active_sessions(object: &serde_json::Map<String, serde_json::Value>) -> Result<Option<u64>> {
    for key in [
        "active_sessions",
        "activeSessions",
        "ActiveSessions",
        "sessions",
        "Sessions",
    ] {
        match object.get(key) {
            Some(serde_json::Value::Number(number)) => {
                return number.as_u64().map(Some).ok_or_else(|| {
                    unparseable("sbx ls", &format!("{key} is not a count of sessions"))
                });
            }
            Some(serde_json::Value::Array(items)) => return Ok(Some(items.len() as u64)),
            Some(serde_json::Value::Null) | None => {}
            Some(_) => {
                return Err(unparseable(
                    "sbx ls",
                    &format!("{key} is neither a count nor a list"),
                ));
            }
        }
    }
    Ok(None)
}

/// `sbx secret ls`が示すsecretの名前。
///
/// 値は取得も表示もしない。存在の有無だけを読む。
pub fn parse_secret_names(output: &str) -> Result<Vec<String>> {
    let documents = json_documents("sbx secret ls", output)?;

    let mut names = Vec::with_capacity(documents.len());
    for document in documents {
        let name = match &document {
            // 名前だけを並べるversionがある。
            serde_json::Value::String(name) => name.clone(),
            serde_json::Value::Object(object) => string_field(object, "name")
                .or_else(|| string_field(object, "Name"))
                .ok_or_else(|| unparseable("sbx secret ls", "an entry has no name"))?,
            _ => return Err(unparseable("sbx secret ls", "an entry has no name")),
        };
        if name.is_empty() {
            return Err(unparseable("sbx secret ls", "an entry has an empty name"));
        }
        names.push(name);
    }
    Ok(names)
}

/// 一覧形式と1行1件のJSON形式のどちらでも読む。
fn json_documents(program: &str, output: &str) -> Result<Vec<serde_json::Value>> {
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

/// `sbx template ls`が示すTemplate 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateEntry {
    pub name: String,
    /// 対応するimage ID。runtimeが対応関係を示さない場合は`None`。
    pub image_id: Option<String>,
}

/// `sbx template ls`のstructured outputをparseする。
///
/// 一覧形式と、1行1件のJSON形式のどちらでも読む。名前を持たないentryがある出力は、
/// 一覧として信用できないためparse不能として扱う。
pub fn parse_template_list(output: &str) -> Result<Vec<TemplateEntry>> {
    let documents = json_documents("sbx template ls", output)?;

    let mut entries = Vec::with_capacity(documents.len());
    for document in documents {
        let object = document
            .as_object()
            .ok_or_else(|| unparseable("sbx template ls", "an entry is not an object"))?;
        let name = string_field(object, "name")
            .or_else(|| string_field(object, "Name"))
            .filter(|name| !name.is_empty())
            .ok_or_else(|| unparseable("sbx template ls", "an entry has no name"))?;
        let image_id = string_field(object, "image_id")
            .or_else(|| string_field(object, "imageId"))
            .or_else(|| string_field(object, "ImageId"))
            .or_else(|| string_field(object, "image"))
            .filter(|id| !id.is_empty());
        entries.push(TemplateEntry { name, image_id });
    }
    Ok(entries)
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
    fn the_sandbox_list_parser_reads_the_fields_the_workflow_compares() {
        let output = r#"[{"name":"sbxm-a","state":"running","workspace":"/tmp/docker-sandboxes/sbxm-a","template":"sbxm-a-template:0123456789ab","active_sessions":2}]"#;
        let entries = parse_sandbox_list(output).expect("a listing parses");
        assert_eq!(
            entries,
            vec![SandboxEntry {
                name: "sbxm-a".to_string(),
                state: SandboxState::Running,
                workspace: Some("/tmp/docker-sandboxes/sbxm-a".to_string()),
                template: Some("sbxm-a-template:0123456789ab".to_string()),
                active_sessions: Some(2),
            }]
        );

        // 1行1件のJSONと、空の出力も同じ意味で読む。
        let lines = "{\"name\":\"sbxm-a\",\"state\":\"stopped\",\"sessions\":[]}\n{\"name\":\"sbxm-b\",\"status\":\"running\",\"sessions\":[{\"id\":\"1\"}]}\n";
        let entries = parse_sandbox_list(lines).expect("line-delimited output parses");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].state, SandboxState::Stopped);
        assert_eq!(entries[0].active_sessions, Some(0));
        assert_eq!(entries[1].active_sessions, Some(1));
        assert!(
            parse_sandbox_list("  \n")
                .expect("an empty listing")
                .is_empty()
        );

        // session数を示さないversionでは、不在を推測しない。
        let entries = parse_sandbox_list(r#"[{"name":"sbxm-a","state":"running"}]"#).unwrap();
        assert_eq!(entries[0].active_sessions, None);
    }

    #[test]
    fn a_sandbox_listing_that_cannot_be_read_is_refused() {
        for output in [
            r#"[{"state":"running"}]"#,
            r#"[{"name":"sbxm-a"}]"#,
            r#"[{"name":"sbxm-a","state":"pausing"}]"#,
            r#"[{"name":"sbxm-a","state":"running","sessions":"two"}]"#,
            r#"["sbxm-a"]"#,
            "true",
        ] {
            let error = parse_sandbox_list(output).expect_err("{output} must be refused");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::ExternalOutputUnparseable),
                "output {output} produced the wrong error"
            );
        }
    }

    #[test]
    fn the_template_list_parser_reads_the_name_and_the_image_it_holds() {
        let entries = parse_template_list(r#"[{"name":"t:1","image_id":"sha256:abc"}]"#)
            .expect("a listing parses");
        assert_eq!(
            entries,
            vec![TemplateEntry {
                name: "t:1".to_string(),
                image_id: Some("sha256:abc".to_string()),
            }]
        );
        // imageを示さないversionでは、対応を推測しない。
        let entries = parse_template_list(r#"[{"name":"t:1"}]"#).unwrap();
        assert_eq!(entries[0].image_id, None);
        assert!(parse_template_list("").unwrap().is_empty());

        for output in [r#"[{"image_id":"sha256:abc"}]"#, r#"[{"name":""}]"#, "12"] {
            let error = parse_template_list(output).expect_err("{output} must be refused");
            assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        }
    }

    #[test]
    fn the_secret_parser_reads_names_only() {
        assert_eq!(
            parse_secret_names(r#"[{"name":"github"},{"name":"other"}]"#).unwrap(),
            vec!["github".to_string(), "other".to_string()]
        );
        assert_eq!(
            parse_secret_names(r#"["github"]"#).unwrap(),
            vec!["github".to_string()]
        );
        assert!(parse_secret_names("").unwrap().is_empty());

        for output in [r#"[{"value":"secret"}]"#, r#"[""]"#, "3"] {
            let error = parse_secret_names(output).expect_err("{output} must be refused");
            assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        }
    }

    #[test]
    fn the_image_inspect_parser_reads_the_identity_and_the_labels() {
        let output = r#"[{"Id":"sha256:abc","Config":{"Labels":{"io.crescware.sbxm.canonical-id":"example-org/example-repo"}}}]"#;
        let identity = parse_image_inspect(output).expect("a single image parses");
        assert_eq!(identity.id, "sha256:abc");
        assert_eq!(
            identity
                .labels
                .get("io.crescware.sbxm.canonical-id")
                .map(String::as_str),
            Some("example-org/example-repo")
        );

        // labelを持たないimageは、labelが空のimageとして読む。
        let identity = parse_image_inspect(r#"[{"Id":"sha256:abc","Config":{"Labels":null}}]"#)
            .expect("an image without labels parses");
        assert!(identity.labels.is_empty());
    }

    #[test]
    fn an_image_inspect_output_that_is_not_one_image_is_refused() {
        for output in [
            "[]",
            r#"[{"Id":"sha256:a","Config":{}},{"Id":"sha256:b","Config":{}}]"#,
            r#"[{"Config":{}}]"#,
            r#"[{"Id":"","Config":{}}]"#,
            r#"[{"Id":"sha256:a"}]"#,
            r#"{"Id":"sha256:a"}"#,
            r#"[{"Id":"sha256:a","Config":{"Labels":{"key":1}}}]"#,
            "not json",
        ] {
            let error = parse_image_inspect(output).expect_err("{output} must be refused");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::ExternalOutputUnparseable),
                "output {output} produced the wrong error"
            );
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
