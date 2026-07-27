//! Project識別子、Sandbox名の導出、project metadata。
//!
//! stringを直接workflowへ渡さず、validation済みの型を使う。metadataと外部状態の
//! validationは作成元や作成履歴を条件にせず、read-only commandとmutation commandで
//! 同じ規則を使う。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;
use crate::paths::{self, AbsoluteBasePath, ProjectPaths};

/// このbuildが読み書きするproject metadataのversion。
pub const METADATA_VERSION: u32 = 1;

/// Sandbox名の最大byte数。
const SANDBOX_NAME_MAX_BYTES: usize = 63;
const SANDBOX_NAME_PREFIX: &str = "sbxm-";
const SANDBOX_NAME_HASH_HEX: usize = 12;

/// `<owner>/<repository>`。
///
/// 比較にはcanonical IDを使い、表示にはGitHub上の表記を使う。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectId {
    owner_display: String,
    repository_display: String,
    canonical_id: String,
}

impl ProjectId {
    /// 1個のslashを持つ`<owner>/<repository>`を検証する。
    pub fn parse(value: &str) -> Result<ProjectId> {
        let invalid = || {
            Error::new(
                ErrorId::InvalidProjectId,
                msg!("error-invalid-project-id", value = value),
            )
        };

        let mut parts = value.split('/');
        let (owner, repository) = match (parts.next(), parts.next(), parts.next()) {
            (Some(owner), Some(repository), None) => (owner, repository),
            _ => return Err(invalid()),
        };

        if !is_valid_owner(owner) || !is_valid_repository(repository) {
            return Err(invalid());
        }
        if repository == "." || repository == ".." {
            return Err(invalid());
        }
        // `.sbxm`はproject metadata directoryと衝突する。
        if repository.eq_ignore_ascii_case(".sbxm") {
            return fail(
                ErrorId::ReservedRepositoryName,
                msg!("error-reserved-repository-name", value = repository),
            );
        }

        Ok(ProjectId {
            owner_display: owner.to_string(),
            repository_display: repository.to_string(),
            canonical_id: format!(
                "{}/{}",
                owner.to_ascii_lowercase(),
                repository.to_ascii_lowercase()
            ),
        })
    }

    /// GitHub上のowner表記。
    pub fn owner_display(&self) -> &str {
        &self.owner_display
    }

    /// GitHub上のrepository表記。
    pub fn repository_display(&self) -> &str {
        &self.repository_display
    }

    /// ASCII lowercaseの`owner/repository`。比較の正本。
    pub fn canonical_id(&self) -> &str {
        &self.canonical_id
    }

    /// case-insensitive filesystem上の重複を避けるためのowner表記。
    pub fn owner_lower(&self) -> String {
        self.owner_display.to_ascii_lowercase()
    }

    /// case-insensitive filesystem上の重複を避けるためのrepository表記。
    pub fn repository_lower(&self) -> String {
        self.repository_display.to_ascii_lowercase()
    }

    /// 表示用の`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        format!("{}/{}", self.owner_display, self.repository_display)
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_id())
    }
}

/// `[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?`
fn is_valid_owner(owner: &str) -> bool {
    let bytes = owner.as_bytes();
    if bytes.is_empty() || bytes.len() > 39 {
        return false;
    }
    let alphanumeric = |b: u8| b.is_ascii_alphanumeric();
    if !alphanumeric(bytes[0]) || !alphanumeric(bytes[bytes.len() - 1]) {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
}

/// `[A-Za-z0-9._-]{1,100}`
fn is_valid_repository(repository: &str) -> bool {
    let bytes = repository.as_bytes();
    if bytes.is_empty() || bytes.len() > 100 {
        return false;
    }
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// canonical project IDから決定的に導出したSandbox名。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SandboxName(String);

impl SandboxName {
    /// canonical project IDからSandbox名を導出する。
    ///
    /// 同じcanonical project IDは常に同じ名前となり、異なるIDは通常hashで区別する。
    pub fn derive(id: &ProjectId) -> SandboxName {
        let canonical = id.canonical_id();
        let hash = sha256_hex(canonical.as_bytes());
        let hash_prefix = &hash[..SANDBOX_NAME_HASH_HEX];

        let mut slug = String::with_capacity(canonical.len());
        for byte in canonical.bytes() {
            let mapped = match byte {
                b'/' => b'-',
                b if b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' => b,
                _ => b'-',
            };
            slug.push(mapped as char);
        }
        let slug = collapse_hyphens(&slug);

        // `sbxm-` + slug + `-` + hash が63 byte以内に収まるようslugの末尾を切る。
        let budget = SANDBOX_NAME_MAX_BYTES - SANDBOX_NAME_PREFIX.len() - 1 - SANDBOX_NAME_HASH_HEX;
        let mut truncated: String = slug.chars().take(budget).collect();
        while truncated.ends_with('-') {
            truncated.pop();
        }

        SandboxName(format!("{SANDBOX_NAME_PREFIX}{truncated}-{hash_prefix}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SandboxName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn collapse_hyphens(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous_hyphen = false;
    for character in value.chars() {
        if character == '-' {
            if !previous_hyphen && !out.is_empty() {
                out.push('-');
            }
            previous_hyphen = true;
        } else {
            out.push(character);
            previous_hyphen = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// SHA-256のlowercase hex。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// managed worktreeのbare rootからの相対path。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedWorktreePath(PathBuf);

impl ManagedWorktreePath {
    /// bare root配下の相対pathだけを受け付ける。
    pub fn new(value: &str) -> std::result::Result<ManagedWorktreePath, &'static str> {
        let path = PathBuf::from(value);
        if value.is_empty() {
            return Err("the path is empty");
        }
        if path.is_absolute() {
            return Err("the path is absolute");
        }
        if path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err("the path contains a parent directory component");
        }
        Ok(ManagedWorktreePath(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// 利用者が要求した目標構成。進捗cacheではない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provisioning {
    pub mode: ProvisioningMode,
    /// attached modeでは、remote default branchを解決するまで空を許可する。
    #[serde(default)]
    pub start_ref: String,
    pub requested_worktrees: u32,
    #[serde(default)]
    pub dockerfile_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProvisioningMode {
    Attached,
    Detached,
}

impl ProvisioningMode {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ProvisioningMode::Attached => "attached",
            ProvisioningMode::Detached => "detached",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktrees {
    #[serde(default)]
    pub managed: Vec<ManagedWorktreeRecord>,
}

/// managed用pathの永続的な宣言。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedWorktreeRecord {
    pub path: String,
    pub created_from: String,
}

/// `rebuild`のSandbox切替中だけ存在する世代判定の正本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildIntent {
    pub target_dockerfile_sha256: String,
    pub previous_dockerfile_sha256: String,
}

/// `<project-root>/.sbxm/project.toml`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub version: u32,
    pub owner: String,
    pub repository: String,
    pub canonical_id: String,
    pub provisioning: Provisioning,
    #[serde(default)]
    pub worktrees: Worktrees,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<RebuildIntent>,
}

impl ProjectMetadata {
    /// metadataが宣言するProject IDを、その場で再検証する。
    pub fn project_id(&self, path: &Path) -> Result<ProjectId> {
        let id =
            ProjectId::parse(&format!("{}/{}", self.owner, self.repository)).map_err(|_| {
                Error::new(
                    ErrorId::MetadataCanonicalIdMismatch,
                    msg!(
                        "error-metadata-canonical-id-mismatch",
                        path = paths::display(path),
                        observed = self.canonical_id,
                        expected = format!("{}/{}", self.owner, self.repository)
                    ),
                )
            })?;
        if id.canonical_id() != self.canonical_id {
            return fail(
                ErrorId::MetadataCanonicalIdMismatch,
                msg!(
                    "error-metadata-canonical-id-mismatch",
                    path = paths::display(path),
                    observed = self.canonical_id,
                    expected = id.canonical_id()
                ),
            );
        }
        Ok(id)
    }
}

/// 探索で見つかった1案件。
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub id: ProjectId,
    pub sandbox_name: SandboxName,
    pub metadata: ProjectMetadata,
    pub metadata_path: PathBuf,
    pub root: PathBuf,
}

impl DiscoveredProject {
    pub fn paths(&self) -> ProjectPaths {
        ProjectPaths::new(self.root.clone())
    }
}

/// base path配下の全案件metadataを読み、canonical IDのbyte昇順で返す。
///
/// 1件の破損を無視して部分的な案件一覧を返さない。canonical ID重複、導出path不一致、
/// Sandbox名衝突は一覧化してerrorとする。
pub fn discover_projects(base: &AbsoluteBasePath) -> Result<Vec<DiscoveredProject>> {
    let mut discovered: Vec<DiscoveredProject> = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    let owner_entries = match read_dir_sorted(base.as_path()) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return fail(
                ErrorId::MetadataUnreadable,
                msg!(
                    "error-metadata-unreadable",
                    path = paths::display(base.as_path()),
                    detail = error
                ),
            );
        }
    };

    for owner_dir in owner_entries {
        // symlinkは追跡しない。
        let Ok(metadata) = fs::symlink_metadata(&owner_dir) else {
            continue;
        };
        if !metadata.is_dir() {
            continue;
        }

        let project_entries = match read_dir_sorted(&owner_dir) {
            Ok(entries) => entries,
            Err(error) => {
                diagnostics.push(Diagnostic::new(
                    ErrorId::MetadataUnreadable,
                    msg!(
                        "error-metadata-unreadable",
                        path = paths::display(&owner_dir),
                        detail = error
                    ),
                ));
                continue;
            }
        };

        for project_root in project_entries {
            let Some(name) = project_root.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.ends_with(".project") {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(&project_root) else {
                continue;
            };
            if !metadata.is_dir() {
                continue;
            }

            let metadata_path = ProjectPaths::new(project_root.clone()).metadata_file();
            if !metadata_path.exists() {
                continue;
            }
            match load_metadata(&metadata_path) {
                Ok(loaded) => {
                    match validate_location(&loaded, &metadata_path, &project_root, base) {
                        Ok(project) => discovered.push(project),
                        Err(error) => diagnostics.extend(error.diagnostics().iter().cloned()),
                    }
                }
                Err(error) => diagnostics.extend(error.diagnostics().iter().cloned()),
            }
        }
    }

    discovered.sort_by(|left, right| left.id.canonical_id().cmp(right.id.canonical_id()));

    // canonical IDの重複と、Sandbox名の衝突を一覧化する。
    let mut by_canonical: BTreeMap<&str, Vec<&PathBuf>> = BTreeMap::new();
    for project in &discovered {
        by_canonical
            .entry(project.id.canonical_id())
            .or_default()
            .push(&project.metadata_path);
    }
    for (canonical_id, locations) in &by_canonical {
        if locations.len() > 1 {
            let joined = locations
                .iter()
                .map(|path| paths::display(path))
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic::new(
                ErrorId::MetadataDuplicateCanonicalId,
                msg!(
                    "error-metadata-duplicate-canonical-id",
                    canonical_id = canonical_id,
                    paths = joined
                ),
            ));
        }
    }

    let mut by_sandbox: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for project in &discovered {
        by_sandbox
            .entry(project.sandbox_name.as_str())
            .or_default()
            .push(project.id.canonical_id());
    }
    for (sandbox_name, canonical_ids) in &by_sandbox {
        let mut distinct: Vec<&&str> = canonical_ids.iter().collect();
        distinct.sort_unstable();
        distinct.dedup();
        if distinct.len() > 1 {
            diagnostics.push(Diagnostic::new(
                ErrorId::SandboxNameCollision,
                msg!(
                    "error-sandbox-name-collision",
                    sandbox_name = sandbox_name,
                    canonical_id = distinct[0],
                    other = distinct[1]
                ),
            ));
        }
    }

    if !diagnostics.is_empty() {
        return Err(Error::many(diagnostics));
    }
    Ok(discovered)
}

fn read_dir_sorted(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut entries: Vec<PathBuf> = fs::read_dir(path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    Ok(entries)
}

/// 1件のmetadata fileを読んで検証する。
pub fn load_metadata(path: &Path) -> Result<ProjectMetadata> {
    if paths::is_symlink(path) {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::MetadataUnreadable,
                msg!(
                    "security-metadata-symlink-description",
                    path = paths::display(path)
                ),
            )
            .remediation(msg!(
                "security-metadata-symlink-remediation",
                path = paths::display(path)
            )),
        ));
    }

    let text = fs::read_to_string(path).map_err(|error| {
        Error::new(
            ErrorId::MetadataUnreadable,
            msg!(
                "error-metadata-unreadable",
                path = paths::display(path),
                detail = error
            ),
        )
    })?;

    let document: toml::Value = toml::from_str(&text).map_err(|error| {
        Error::new(
            ErrorId::MetadataInvalidSyntax,
            msg!(
                "error-metadata-invalid-syntax",
                path = paths::display(path),
                detail = error.message()
            ),
        )
    })?;

    // versionの解釈を先に確定させ、未知versionを他の項目より前に診断する。
    match document.get("version").and_then(|value| value.as_integer()) {
        Some(version) if version == i64::from(METADATA_VERSION) => {}
        Some(version) => {
            return fail(
                ErrorId::MetadataUnknownVersion,
                msg!(
                    "error-metadata-unknown-version",
                    path = paths::display(path),
                    version = version,
                    supported = METADATA_VERSION
                ),
            );
        }
        None => {
            return fail(
                ErrorId::MetadataMissingField,
                msg!(
                    "error-metadata-missing-field",
                    path = paths::display(path),
                    field = "version"
                ),
            );
        }
    }

    let metadata: ProjectMetadata = document.try_into().map_err(|error: toml::de::Error| {
        Error::new(
            ErrorId::MetadataMissingField,
            msg!(
                "error-metadata-missing-field",
                path = paths::display(path),
                field = error.message()
            ),
        )
    })?;

    for record in &metadata.worktrees.managed {
        if let Err(detail) = ManagedWorktreePath::new(&record.path) {
            return fail(
                ErrorId::MetadataInvalidSyntax,
                msg!(
                    "error-metadata-invalid-syntax",
                    path = paths::display(path),
                    detail = format!("worktrees.managed path {}: {detail}", record.path)
                ),
            );
        }
    }

    Ok(metadata)
}

/// metadataが宣言する案件が、実際にその場所へ置かれているかを検証する。
fn validate_location(
    metadata: &ProjectMetadata,
    metadata_path: &Path,
    project_root: &Path,
    base: &AbsoluteBasePath,
) -> Result<DiscoveredProject> {
    let id = metadata.project_id(metadata_path)?;
    let expected_root = base.project_root(&id);
    if expected_root != project_root {
        return fail(
            ErrorId::MetadataPathMismatch,
            msg!(
                "error-metadata-path-mismatch",
                path = paths::display(metadata_path),
                expected = paths::display(&expected_root)
            ),
        );
    }
    Ok(DiscoveredProject {
        sandbox_name: SandboxName::derive(&id),
        id,
        metadata: metadata.clone(),
        metadata_path: metadata_path.to_path_buf(),
        root: project_root.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_project(base: &Path, owner: &str, repository: &str, body: &str) -> PathBuf {
        let root = base
            .join(owner.to_ascii_lowercase())
            .join(format!("{}.project", repository.to_ascii_lowercase()));
        let sbxm = root.join(".sbxm");
        fs::create_dir_all(&sbxm).expect("create project directories");
        let path = sbxm.join("project.toml");
        fs::write(&path, body).expect("write metadata");
        path
    }

    fn metadata_body(owner: &str, repository: &str) -> String {
        format!(
            r#"version = 1
owner = "{owner}"
repository = "{repository}"
canonical_id = "{}/{}"

[provisioning]
mode = "attached"
start_ref = "main"
requested_worktrees = 1
dockerfile_sha256 = "abc"
"#,
            owner.to_ascii_lowercase(),
            repository.to_ascii_lowercase()
        )
    }

    #[test]
    fn project_ids_keep_display_casing_and_compare_by_canonical_form() {
        let id = ProjectId::parse("Example-Org/Example.Repo").expect("valid");
        assert_eq!(id.owner_display(), "Example-Org");
        assert_eq!(id.repository_display(), "Example.Repo");
        assert_eq!(id.canonical_id(), "example-org/example.repo");
        assert_eq!(id.display_id(), "Example-Org/Example.Repo");
        assert_eq!(id.owner_lower(), "example-org");
        assert_eq!(id.repository_lower(), "example.repo");
    }

    #[test]
    fn project_ids_require_exactly_one_slash() {
        for value in ["owner", "owner/repo/extra", "/repo", "owner/", ""] {
            let error = ProjectId::parse(value).expect_err("{value} must be rejected");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::InvalidProjectId),
                "value {value} produced the wrong error"
            );
        }
    }

    #[test]
    fn owner_and_repository_character_rules_are_enforced() {
        assert!(ProjectId::parse("a/b").is_ok());
        assert!(ProjectId::parse("a-b-c/repo_name.v2").is_ok());
        assert!(ProjectId::parse(&format!("{}/repo", "a".repeat(39))).is_ok());
        assert!(ProjectId::parse(&format!("{}/repo", "a".repeat(40))).is_err());
        assert!(ProjectId::parse("-owner/repo").is_err());
        assert!(ProjectId::parse("owner-/repo").is_err());
        assert!(ProjectId::parse("own.er/repo").is_err());
        assert!(ProjectId::parse(&format!("owner/{}", "r".repeat(100))).is_ok());
        assert!(ProjectId::parse(&format!("owner/{}", "r".repeat(101))).is_err());
        assert!(ProjectId::parse("owner/re po").is_err());
    }

    #[test]
    fn dot_and_dot_dot_are_not_repository_names() {
        assert!(ProjectId::parse("owner/.").is_err());
        assert!(ProjectId::parse("owner/..").is_err());
    }

    #[test]
    fn the_metadata_directory_name_is_reserved_case_insensitively() {
        for value in ["owner/.sbxm", "owner/.SBXM", "owner/.Sbxm"] {
            let error = ProjectId::parse(value).expect_err("reserved names are rejected");
            assert_eq!(error.first_id(), Some(ErrorId::ReservedRepositoryName));
        }
    }

    #[test]
    fn sandbox_names_are_deterministic_and_case_insensitive() {
        let lower = SandboxName::derive(&ProjectId::parse("owner/repo").unwrap());
        let upper = SandboxName::derive(&ProjectId::parse("OWNER/REPO").unwrap());
        assert_eq!(lower, upper);
        assert!(lower.as_str().starts_with("sbxm-owner-repo-"));
        assert_eq!(lower.as_str().len(), "sbxm-owner-repo-".len() + 12);
    }

    #[test]
    fn sandbox_names_separate_projects_that_share_a_slug() {
        // `owner/repo`と`owner-repo`は同じslugになるが、hashで区別される。
        let first = SandboxName::derive(&ProjectId::parse("owner/repo").unwrap());
        let second = SandboxName::derive(&ProjectId::parse("owner-repo/x").unwrap());
        assert_ne!(first, second);
    }

    #[test]
    fn sandbox_names_stay_within_63_bytes() {
        let id = ProjectId::parse(&format!("{}/{}", "o".repeat(39), "r".repeat(100))).unwrap();
        let name = SandboxName::derive(&id);
        assert!(
            name.as_str().len() <= 63,
            "{} is {} bytes",
            name.as_str(),
            name.as_str().len()
        );
        assert!(name.as_str().starts_with("sbxm-"));
        // 切り詰めてもhash suffixは常に残る。
        let hash = &name.as_str()[name.as_str().len() - 12..];
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sandbox_name_characters_are_restricted_to_the_documented_alphabet() {
        let id = ProjectId::parse("Example-Org/Example.Repo_v2").unwrap();
        let name = SandboxName::derive(&id);
        assert!(
            name.as_str()
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "{name} contains characters outside [a-z0-9-]"
        );
        assert!(!name.as_str().contains("--"));
    }

    #[test]
    fn sha256_matches_the_reference_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn managed_worktree_paths_stay_inside_the_bare_root() {
        assert!(ManagedWorktreePath::new("repo.tree-0").is_ok());
        assert!(ManagedWorktreePath::new("").is_err());
        assert!(ManagedWorktreePath::new("/absolute").is_err());
        assert!(ManagedWorktreePath::new("../escape").is_err());
        assert!(ManagedWorktreePath::new("nested/../..").is_err());
    }

    #[test]
    fn metadata_round_trips_through_toml() {
        let metadata = ProjectMetadata {
            version: 1,
            owner: "Example-Org".into(),
            repository: "Example-Repo".into(),
            canonical_id: "example-org/example-repo".into(),
            provisioning: Provisioning {
                mode: ProvisioningMode::Detached,
                start_ref: "develop".into(),
                requested_worktrees: 3,
                dockerfile_sha256: "0123456789ab".into(),
            },
            worktrees: Worktrees {
                managed: vec![ManagedWorktreeRecord {
                    path: "example-repo.tree-0".into(),
                    created_from: "refs/remotes/origin/develop".into(),
                }],
            },
            rebuild: None,
        };

        let serialized = toml::to_string(&metadata).expect("serialize");
        assert!(
            !serialized.contains("[rebuild]"),
            "a project without a rebuild intent must not record one: {serialized}"
        );
        let parsed: ProjectMetadata = toml::from_str(&serialized).expect("deserialize");
        assert_eq!(parsed, metadata);
    }

    #[test]
    fn metadata_rejects_an_unknown_version_before_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.toml");
        fs::write(&path, "version = 2\n").unwrap();
        let error = load_metadata(&path).expect_err("unknown versions are refused");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataUnknownVersion));
    }

    #[test]
    fn metadata_rejects_a_canonical_id_that_does_not_match_its_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.toml");
        fs::write(
            &path,
            r#"version = 1
owner = "example-org"
repository = "example-repo"
canonical_id = "other-org/other-repo"

[provisioning]
mode = "attached"
start_ref = "main"
requested_worktrees = 1
dockerfile_sha256 = "abc"
"#,
        )
        .unwrap();
        let metadata = load_metadata(&path).expect("syntax is valid");
        let error = metadata
            .project_id(&path)
            .expect_err("the declared canonical id must match the fields");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataCanonicalIdMismatch));
    }

    #[test]
    fn discovery_returns_projects_sorted_by_canonical_id() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        write_project(dir.path(), "zeta", "repo", &metadata_body("zeta", "repo"));
        write_project(dir.path(), "alpha", "repo", &metadata_body("alpha", "repo"));

        let projects = discover_projects(&base).expect("discovery succeeds");
        let ids: Vec<&str> = projects.iter().map(|p| p.id.canonical_id()).collect();
        assert_eq!(ids, vec!["alpha/repo", "zeta/repo"]);
    }

    #[test]
    fn discovery_of_an_empty_base_path_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        assert!(discover_projects(&base).unwrap().is_empty());
    }

    #[test]
    fn a_single_broken_project_stops_the_whole_listing() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        write_project(dir.path(), "alpha", "repo", &metadata_body("alpha", "repo"));
        write_project(dir.path(), "broken", "repo", "version = 1\nowner = ");

        let error = discover_projects(&base).expect_err("a broken project fails the listing");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidSyntax));
    }

    #[test]
    fn metadata_placed_at_the_wrong_path_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        // ownerディレクトリと宣言ownerが食い違う配置。
        write_project(dir.path(), "other", "repo", &metadata_body("alpha", "repo"));

        let error = discover_projects(&base).expect_err("misplaced metadata is refused");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataPathMismatch));
    }

    #[test]
    fn discovery_does_not_follow_symlinked_project_directories() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();

        write_project(
            outside.path(),
            "alpha",
            "repo",
            &metadata_body("alpha", "repo"),
        );
        std::os::unix::fs::symlink(outside.path().join("alpha"), dir.path().join("alpha"))
            .expect("symlink the owner directory");

        let projects = discover_projects(&base).expect("discovery succeeds");
        assert!(
            projects.is_empty(),
            "symlinked owner directories are not followed"
        );
    }

    #[test]
    fn metadata_files_that_are_symlinks_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.toml");
        fs::write(&real, metadata_body("alpha", "repo")).unwrap();
        let link = dir.path().join("project.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let error = load_metadata(&link).expect_err("symlinked metadata is refused");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
    }
}
