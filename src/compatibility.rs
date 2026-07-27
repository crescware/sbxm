//! Docker Sandboxes CLIとの互換性契約。
//!
//! Docker Sandboxes CLIはEarly Accessであるため、参照資料の現在内容ではなく、
//! 対象Macで採取してcommitしたexact-version fixtureを実装上の契約とする。
//! 安全性に必要な出力を解釈できないversionではmutationを行わない。

use std::path::{Path, PathBuf};

use serde::Deserialize;

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

const MANIFEST_SOURCE: &str = include_str!("../compatibility.toml");

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
    /// 出力書式は対象versionのfixtureで確定する。ここでは数値3要素の並びだけを認め、
    /// 見つからない場合はparse不能として扱う。
    pub fn extract_from_output(output: &str) -> Option<CliVersion> {
        for token in output.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
            if let Some(version) = CliVersion::parse(token) {
                return Some(version);
            }
        }
        None
    }

    /// major/minorが同じで、patchだけが異なるか。
    pub fn differs_only_in_patch(&self, other: &CliVersion) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch != other.patch
    }
}

impl std::fmt::Display for CliVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    schema_version: u32,
    validated_cli_versions: Vec<String>,
    ls_json_fixture_version: u32,
}

/// commitした互換性manifest。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityManifest {
    pub schema_version: u32,
    pub validated_cli_versions: Vec<CliVersion>,
    pub ls_json_fixture_version: u32,
}

impl CompatibilityManifest {
    /// build時に埋め込んだmanifestを読む。
    ///
    /// manifestの不備はbuild成果物の不備であり、testで検出する。
    pub fn embedded() -> CompatibilityManifest {
        CompatibilityManifest::parse(MANIFEST_SOURCE)
            .unwrap_or_else(|error| panic!("embedded compatibility manifest is invalid: {error}"))
    }

    fn parse(source: &str) -> std::result::Result<CompatibilityManifest, String> {
        let raw: RawManifest = toml::from_str(source).map_err(|error| error.to_string())?;
        if raw.schema_version != 1 {
            return Err(format!("unsupported schema_version {}", raw.schema_version));
        }
        let mut validated = Vec::with_capacity(raw.validated_cli_versions.len());
        for value in &raw.validated_cli_versions {
            let version = CliVersion::parse(value)
                .ok_or_else(|| format!("{value} is not an exact three-part version"))?;
            validated.push(version);
        }
        Ok(CompatibilityManifest {
            schema_version: raw.schema_version,
            validated_cli_versions: validated,
            ls_json_fixture_version: raw.ls_json_fixture_version,
        })
    }

    /// 実機採取済みのfixtureがあるか。
    pub fn has_validated_versions(&self) -> bool {
        !self.validated_cli_versions.is_empty()
    }

    fn validated_list(&self) -> String {
        self.validated_cli_versions
            .iter()
            .map(|version| version.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// 検出したversionを互換性判定へ写像する。
    pub fn classify(&self, observed: CliVersion) -> Compatibility {
        if observed < MINIMUM_CLI_VERSION {
            return Compatibility::BelowMinimum { observed };
        }
        if !self.has_validated_versions() {
            return Compatibility::FixturesNotCollected { observed };
        }
        if self.validated_cli_versions.contains(&observed) {
            return Compatibility::Validated { observed };
        }
        if self
            .validated_cli_versions
            .iter()
            .any(|validated| observed.differs_only_in_patch(validated))
        {
            return Compatibility::PatchDrift {
                observed,
                validated: self.validated_list(),
            };
        }
        Compatibility::Unsupported {
            observed,
            validated: self.validated_list(),
        }
    }
}

/// 検出したversionの互換性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compatibility {
    /// fixtureと一致するversion。
    Validated { observed: CliVersion },
    /// patch versionだけ異なる。read-onlyはwarning付きで許可し、mutationは拒否する。
    PatchDrift {
        observed: CliVersion,
        validated: String,
    },
    /// minor/majorが異なる。
    Unsupported {
        observed: CliVersion,
        validated: String,
    },
    /// 0.37.0未満。
    BelowMinimum { observed: CliVersion },
    /// このbuildに実機fixtureがない。
    FixturesNotCollected { observed: CliVersion },
}

impl Compatibility {
    pub fn observed(&self) -> CliVersion {
        match self {
            Compatibility::Validated { observed }
            | Compatibility::PatchDrift { observed, .. }
            | Compatibility::Unsupported { observed, .. }
            | Compatibility::BelowMinimum { observed }
            | Compatibility::FixturesNotCollected { observed } => *observed,
        }
    }

    /// read-only commandを続行してよいか。
    pub fn allows_read_only(&self) -> bool {
        matches!(
            self,
            Compatibility::Validated { .. } | Compatibility::PatchDrift { .. }
        )
    }

    /// 状態を変更する操作を許可してよいか。
    pub fn allows_mutation(&self) -> bool {
        matches!(self, Compatibility::Validated { .. })
    }

    /// read-only文脈で表示するwarning。
    pub fn warning(&self) -> Option<crate::error::Msg> {
        match self {
            Compatibility::PatchDrift {
                observed,
                validated,
            } => Some(msg!(
                "warning-sbx-version-patch-drift",
                observed = observed,
                validated = validated
            )),
            _ => None,
        }
    }

    /// read-only文脈で続行できない場合のerror。
    pub fn read_only_error(&self) -> Option<Error> {
        match self {
            Compatibility::Validated { .. } | Compatibility::PatchDrift { .. } => None,
            Compatibility::BelowMinimum { observed } => Some(Error::new(
                ErrorId::SbxVersionBelowMinimum,
                msg!(
                    "error-sbx-version-below-minimum",
                    observed = observed,
                    minimum = MINIMUM_CLI_VERSION
                ),
            )),
            Compatibility::Unsupported {
                observed,
                validated,
            } => Some(Error::new(
                ErrorId::SbxVersionUnsupported,
                msg!(
                    "error-sbx-version-unsupported",
                    observed = observed,
                    validated = validated
                ),
            )),
            Compatibility::FixturesNotCollected { observed } => Some(Error::single(
                Diagnostic::new(
                    ErrorId::SbxFixturesNotCollected,
                    msg!("error-sbx-fixtures-not-collected", observed = observed),
                )
                .remediation(msg!(
                    "remediation-collect-fixtures",
                    path = FIXTURE_ROOT_HINT
                )),
            )),
        }
    }

    /// mutation文脈で続行できない場合のerror。
    pub fn mutation_error(&self) -> Option<Error> {
        match self {
            Compatibility::Validated { .. } => None,
            Compatibility::PatchDrift {
                observed,
                validated,
            } => Some(Error::new(
                ErrorId::SbxVersionPatchDrift,
                msg!(
                    "error-sbx-version-patch-drift",
                    observed = observed,
                    validated = validated
                ),
            )),
            other => other.read_only_error(),
        }
    }
}

/// fixtureの置き場所。診断の対処方法として表示する。
pub const FIXTURE_ROOT_HINT: &str = "tests/fixtures/sbx/<version>/";

/// 採取済みfixtureの読み込み。
///
/// fixtureは、それを使用するcommandのPRへ同時に追加・更新する。
#[derive(Debug, Clone)]
pub struct FixtureSet {
    root: PathBuf,
}

impl FixtureSet {
    /// repository内のfixture rootを返す。
    pub fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sbx")
    }

    /// 実機で採取したexact versionのfixture。
    pub fn for_version(version: CliVersion) -> FixtureSet {
        FixtureSet {
            root: FixtureSet::root().join(version.to_string()),
        }
    }

    /// 実機fixtureではない合成データ。parserの境界動作の検証だけに使う。
    pub fn synthetic() -> FixtureSet {
        FixtureSet {
            root: FixtureSet::root().join("synthetic"),
        }
    }

    pub fn exists(&self) -> bool {
        self.root.is_dir()
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn load(&self, name: &str) -> std::io::Result<String> {
        std::fs::read_to_string(self.path(name))
    }
}

/// `sbx ls --json`の1 entry。
///
/// state値の3値への正規化はPhase 3の責務であり、ここではraw valueをそのまま保持する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxListEntry {
    pub name: String,
    pub raw_state: String,
    pub workspace: Option<String>,
}

/// `sbx ls --json`をparseする。
///
/// name、stateが揃わないentryはparse不能とし、推測で補完しない。
pub fn parse_sandbox_list(output: &str) -> Result<Vec<SandboxListEntry>> {
    let document: serde_json::Value = serde_json::from_str(output)
        .map_err(|error| unparseable("sbx ls --json", &error.to_string()))?;

    let items = match &document {
        serde_json::Value::Array(items) => items.clone(),
        serde_json::Value::Object(object) => match object.get("sandboxes") {
            Some(serde_json::Value::Array(items)) => items.clone(),
            _ => {
                return Err(unparseable(
                    "sbx ls --json",
                    "the object has no sandboxes array",
                ));
            }
        },
        _ => {
            return Err(unparseable(
                "sbx ls --json",
                "the document is neither an array nor an object",
            ));
        }
    };

    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let object = item
            .as_object()
            .ok_or_else(|| unparseable("sbx ls --json", "an entry is not an object"))?;
        let name = string_field(object, "name")
            .ok_or_else(|| unparseable("sbx ls --json", "an entry has no name"))?;
        let raw_state = string_field(object, "state")
            .or_else(|| string_field(object, "status"))
            .ok_or_else(|| unparseable("sbx ls --json", "an entry has no state"))?;
        let workspace = string_field(object, "workspace");
        entries.push(SandboxListEntry {
            name,
            raw_state,
            workspace,
        });
    }
    Ok(entries)
}

/// daemonの状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Running,
    Stopped,
}

impl DaemonState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            DaemonState::Running => "running",
            DaemonState::Stopped => "stopped",
        }
    }
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

/// network policyが検証済みbaselineと完全一致するかを判定する。
pub fn require_expected_network_policy(observed: &str) -> Result<()> {
    if observed == EXPECTED_NETWORK_POLICY {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::NetworkPolicyMismatch,
            msg!(
                "error-network-policy-mismatch",
                observed = observed,
                expected = EXPECTED_NETWORK_POLICY
            ),
        )
        .remediation(msg!(
            "remediation-network-policy",
            expected = EXPECTED_NETWORK_POLICY
        )),
    ))
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

    fn manifest(versions: &[&str]) -> CompatibilityManifest {
        CompatibilityManifest {
            schema_version: 1,
            validated_cli_versions: versions
                .iter()
                .map(|value| CliVersion::parse(value).unwrap())
                .collect(),
            ls_json_fixture_version: 1,
        }
    }

    #[test]
    fn the_embedded_manifest_parses() {
        let manifest = CompatibilityManifest::embedded();
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.ls_json_fixture_version, 1);
    }

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
        let classification = manifest(&["0.37.0"]).classify(CliVersion::parse("0.36.9").unwrap());
        assert!(matches!(classification, Compatibility::BelowMinimum { .. }));
        assert!(!classification.allows_read_only());
        assert!(!classification.allows_mutation());
        assert_eq!(
            classification.read_only_error().unwrap().first_id(),
            Some(ErrorId::SbxVersionBelowMinimum)
        );
    }

    #[test]
    fn a_validated_version_allows_both_read_only_and_mutation() {
        let classification = manifest(&["0.37.0"]).classify(CliVersion::parse("0.37.0").unwrap());
        assert!(classification.allows_read_only());
        assert!(classification.allows_mutation());
        assert!(classification.warning().is_none());
        assert!(classification.read_only_error().is_none());
        assert!(classification.mutation_error().is_none());
    }

    #[test]
    fn a_patch_difference_allows_read_only_with_a_warning_but_refuses_mutation() {
        let classification = manifest(&["0.37.0"]).classify(CliVersion::parse("0.37.5").unwrap());
        assert!(classification.allows_read_only());
        assert!(!classification.allows_mutation());
        assert_eq!(
            classification.warning().unwrap().id,
            "warning-sbx-version-patch-drift"
        );
        assert!(classification.read_only_error().is_none());
        assert_eq!(
            classification.mutation_error().unwrap().first_id(),
            Some(ErrorId::SbxVersionPatchDrift)
        );
    }

    #[test]
    fn a_minor_or_major_difference_is_unsupported() {
        for observed in ["0.38.0", "1.37.0"] {
            let classification =
                manifest(&["0.37.0"]).classify(CliVersion::parse(observed).unwrap());
            assert!(
                matches!(classification, Compatibility::Unsupported { .. }),
                "{observed} must be unsupported"
            );
            assert!(!classification.allows_read_only());
            assert!(!classification.allows_mutation());
        }
    }

    #[test]
    fn without_collected_fixtures_no_version_is_interpreted() {
        let classification = manifest(&[]).classify(CliVersion::parse("0.37.0").unwrap());
        assert!(matches!(
            classification,
            Compatibility::FixturesNotCollected { .. }
        ));
        assert!(!classification.allows_read_only());
        assert!(!classification.allows_mutation());
        let error = classification.read_only_error().unwrap();
        assert_eq!(error.first_id(), Some(ErrorId::SbxFixturesNotCollected));
        assert!(error.diagnostics()[0].remediation.is_some());
    }

    #[test]
    fn this_build_has_no_validated_versions_yet() {
        // 実機採取が完了したPRでこのtestを更新し、採取したversionを固定する。
        assert!(
            !CompatibilityManifest::embedded().has_validated_versions(),
            "update this test together with the collected fixtures"
        );
    }

    #[test]
    fn the_sandbox_list_parser_accepts_the_synthetic_shapes() {
        let fixtures = FixtureSet::synthetic();

        let empty = fixtures.load("ls-empty.json").expect("fixture is present");
        assert!(parse_sandbox_list(&empty).unwrap().is_empty());

        let running = fixtures
            .load("ls-running.json")
            .expect("fixture is present");
        let entries = parse_sandbox_list(&running).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "sbxm-owner-repo-0123456789ab");
        assert_eq!(entries[0].raw_state, "running");
        assert_eq!(
            entries[0].workspace.as_deref(),
            Some("/tmp/docker-sandboxes/sbxm-owner-repo-0123456789ab")
        );

        let stopped = fixtures
            .load("ls-stopped.json")
            .expect("fixture is present");
        let entries = parse_sandbox_list(&stopped).unwrap();
        assert_eq!(entries[0].raw_state, "stopped");
    }

    #[test]
    fn the_sandbox_list_parser_refuses_incomplete_entries() {
        for output in [
            "not json",
            "{}",
            "42",
            r#"[{"state":"running"}]"#,
            r#"[{"name":"sbxm-a"}]"#,
            r#"[["name","state"]]"#,
        ] {
            let error = parse_sandbox_list(output)
                .expect_err("incomplete output must not be treated as an empty list");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::ExternalOutputUnparseable),
                "output {output} produced the wrong error"
            );
        }
    }

    #[test]
    fn the_daemon_status_parser_maps_only_known_states() {
        let fixtures = FixtureSet::synthetic();
        let running = fixtures.load("daemon-status-running.json").unwrap();
        assert_eq!(parse_daemon_status(&running).unwrap(), DaemonState::Running);

        let stopped = fixtures.load("daemon-status-stopped.json").unwrap();
        assert_eq!(parse_daemon_status(&stopped).unwrap(), DaemonState::Stopped);

        for output in ["{}", r#"{"state":"degraded"}"#, "[]", "oops"] {
            let error = parse_daemon_status(output).expect_err("unknown states are not guessed");
            assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        }
    }

    #[test]
    fn the_network_policy_parser_reads_the_active_entry_only() {
        let fixtures = FixtureSet::synthetic();
        let balanced = fixtures.load("policy-ls-balanced.json").unwrap();
        assert_eq!(parse_network_policy(&balanced).unwrap(), "Balanced");

        let other = fixtures.load("policy-ls-unsupported.json").unwrap();
        let observed = parse_network_policy(&other).unwrap();
        assert_ne!(observed, EXPECTED_NETWORK_POLICY);

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

    #[test]
    fn only_the_validated_baseline_policy_is_accepted() {
        assert!(require_expected_network_policy("Balanced").is_ok());
        for observed in ["Restricted", "Open", "balanced", ""] {
            let error = require_expected_network_policy(observed)
                .expect_err("{observed} must not be accepted");
            assert_eq!(error.first_id(), Some(ErrorId::NetworkPolicyMismatch));
            assert!(error.diagnostics()[0].remediation.is_some());
        }
    }

    #[test]
    fn the_fixture_root_points_at_the_repository() {
        assert!(
            FixtureSet::root().ends_with("tests/fixtures/sbx"),
            "{}",
            FixtureSet::root().display()
        );
        assert!(FixtureSet::synthetic().exists());
    }
}
