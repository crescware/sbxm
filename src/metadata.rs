//! Project metadata `<project-root>/.sbxm/project.toml`。
//!
//! metadataは進捗cacheではなく、利用者が要求した目標構成である。sbxmは成果物の
//! 作成元を追跡しないため、validation規則を満たすmetadataは、誰が書いたかを問わず
//! 同じものとして扱う。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::git;
use crate::msg;
use crate::paths::{
    self, AbsoluteBasePath, PRIVATE_FILE_MODE, PROJECT_DIR_SUFFIX, ProjectPaths, atomic_create,
    atomic_replace,
};
use crate::project::{CanonicalProjectId, ProjectId, SandboxName};

/// このbuildが読み書きするmetadataのversion。
pub const METADATA_VERSION: u32 = 1;

/// managed worktreeの下限と上限。CLIのoption validationと同じ範囲を使う。
pub const MIN_WORKTREES: u32 = 1;
pub const MAX_WORKTREES: u32 = 32;

/// 全managed worktreeの作り方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationMode {
    /// remote default branchをtrackingするlocal branchを1つ作る。
    Attached,
    /// 全managed worktreeを同じ`origin/<BRANCH>` commitから作る。
    Detached,
}

impl CreationMode {
    /// 翻訳しない安定した表記。metadataと利用者向けtableの両方で使う。
    pub fn as_str(self) -> &'static str {
        match self {
            CreationMode::Attached => "attached",
            CreationMode::Detached => "detached",
        }
    }

    fn parse(value: &str) -> Option<CreationMode> {
        match value {
            "attached" => Some(CreationMode::Attached),
            "detached" => Some(CreationMode::Detached),
            _ => None,
        }
    }
}

impl std::fmt::Display for CreationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 利用者が要求した目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provisioning {
    pub mode: CreationMode,
    /// 起点branch。attached modeではremote default branchを解決するまで未確定を許す。
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
    /// 初回構築中は採用世代、構築完了後は適用済みのDockerfile hash。
    pub dockerfile_sha256: String,
}

/// 作成済みのmanaged worktree。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedWorktree {
    /// bare rootからの相対path。
    pub path: String,
    /// `refs/remotes/origin/<branch>`
    pub created_from: String,
}

/// `rebuild`のSandbox切替中だけ存在する適用予定世代。
///
/// 記録するのは`rebuild`を実装するPhaseであり、本buildは読むだけである。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildIntent {
    pub target_dockerfile_sha256: String,
    pub previous_dockerfile_sha256: String,
}

/// 1案件のmetadata。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMetadata {
    /// GitHub上の表記のままのowner。
    pub owner: String,
    /// GitHub上の表記のままのrepository。
    pub repository: String,
    pub canonical_id: CanonicalProjectId,
    pub provisioning: Provisioning,
    pub managed_worktrees: Vec<ManagedWorktree>,
    pub rebuild: Option<RebuildIntent>,
}

impl ProjectMetadata {
    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        format!("{}/{}", self.owner, self.repository)
    }

    /// canonical project IDから決定的に導出したSandbox名。
    pub fn sandbox_name(&self) -> SandboxName {
        SandboxName::derive(&self.canonical_id)
    }
}

/// 探索で見つかった1案件。
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
}

/// TOMLの生表現。structへ変換する前に必須項目と値を検査する。
#[derive(Debug, Deserialize)]
struct RawMetadata {
    version: Option<i64>,
    owner: Option<String>,
    repository: Option<String>,
    canonical_id: Option<String>,
    provisioning: Option<RawProvisioning>,
    worktrees: Option<RawWorktrees>,
    rebuild: Option<RawRebuild>,
}

#[derive(Debug, Deserialize)]
struct RawProvisioning {
    mode: Option<String>,
    start_ref: Option<String>,
    requested_worktrees: Option<i64>,
    dockerfile_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawWorktrees {
    #[serde(default)]
    managed: Vec<RawManagedWorktree>,
}

#[derive(Debug, Deserialize)]
struct RawManagedWorktree {
    path: Option<String>,
    created_from: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRebuild {
    target_dockerfile_sha256: Option<String>,
    previous_dockerfile_sha256: Option<String>,
}

/// metadataをread-onlyで読む。存在しなければ`None`。
pub fn load(paths: &ProjectPaths) -> Result<Option<ProjectMetadata>> {
    let path = paths.metadata_file();
    match read_optional(&path)? {
        Some(text) => Ok(Some(parse(&text, &path)?)),
        None => Ok(None),
    }
}

/// metadataを新規作成する。既存fileは上書きしない。
pub fn create(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<()> {
    atomic_create(&paths.metadata_file(), &render(metadata), PRIVATE_FILE_MODE)
}

/// 既存metadataをatomicに置き換える。
pub fn update(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Result<()> {
    atomic_replace(&paths.metadata_file(), &render(metadata), PRIVATE_FILE_MODE)
}

/// symlinkを追跡せずにmetadataを読む。
fn read_optional(path: &Path) -> Result<Option<String>> {
    if paths::is_symlink(path) {
        // symlinkの先は案件directory外にあり得るため、追跡せず不在として扱わない。
        return fail(
            ErrorId::MetadataUnreadable,
            msg!(
                "error-metadata-unreadable",
                path = paths::display(path),
                detail = "the metadata path is a symbolic link"
            ),
        );
    }
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => fail(
            ErrorId::MetadataUnreadable,
            msg!(
                "error-metadata-unreadable",
                path = paths::display(path),
                detail = error
            ),
        ),
    }
}

/// metadataのtextを検証する。
pub fn parse(text: &str, path: &Path) -> Result<ProjectMetadata> {
    let raw: RawMetadata = toml::from_str(text).map_err(|error: toml::de::Error| {
        Error::new(
            ErrorId::MetadataInvalidSyntax,
            msg!(
                "error-metadata-invalid-syntax",
                path = paths::display(path),
                detail = error.message()
            ),
        )
    })?;

    let missing = |field: &'static str| {
        Error::new(
            ErrorId::MetadataMissingField,
            msg!(
                "error-metadata-missing-field",
                path = paths::display(path),
                field = field
            ),
        )
    };
    let invalid = |field: &'static str, detail: String| {
        Error::new(
            ErrorId::MetadataInvalidValue,
            msg!(
                "error-metadata-invalid-value",
                path = paths::display(path),
                field = field,
                detail = detail
            ),
        )
    };

    // versionを最初に確定させ、未知versionを他の項目より前に診断する。
    match raw.version {
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
        None => return Err(missing("version")),
    }

    let owner = raw.owner.ok_or_else(|| missing("owner"))?;
    let repository = raw.repository.ok_or_else(|| missing("repository"))?;
    let canonical_value = raw.canonical_id.ok_or_else(|| missing("canonical_id"))?;

    let display_id = ProjectId::parse(&format!("{owner}/{repository}"))
        .map_err(|_| invalid("owner", format!("{owner}/{repository} is not a project ID")))?;
    let canonical_id = CanonicalProjectId::parse(&canonical_value).map_err(|_| {
        invalid(
            "canonical_id",
            format!("{canonical_value} is not a canonical project ID"),
        )
    })?;
    if display_id.canonical() != canonical_id {
        return Err(invalid(
            "canonical_id",
            format!(
                "{canonical_id} does not match owner and repository, which fold to {}",
                display_id.canonical()
            ),
        ));
    }

    let provisioning = raw.provisioning.ok_or_else(|| missing("provisioning"))?;
    let mode_value = provisioning
        .mode
        .ok_or_else(|| missing("provisioning.mode"))?;
    let mode = CreationMode::parse(&mode_value).ok_or_else(|| {
        invalid(
            "provisioning.mode",
            format!(
                "{mode_value} is neither {} nor {}",
                CreationMode::Attached,
                CreationMode::Detached
            ),
        )
    })?;

    let start_ref_value = provisioning
        .start_ref
        .ok_or_else(|| missing("provisioning.start_ref"))?;
    let start_ref = if start_ref_value.is_empty() {
        // attached modeだけが、remote default branchの解決前を表せる。
        if mode == CreationMode::Detached {
            return Err(invalid(
                "provisioning.start_ref",
                format!("{} mode requires an explicit start branch", mode),
            ));
        }
        None
    } else {
        git::validate_branch_name(&start_ref_value).map_err(|_| {
            invalid(
                "provisioning.start_ref",
                format!("{start_ref_value} is not a branch name"),
            )
        })?;
        Some(start_ref_value)
    };

    let requested = provisioning
        .requested_worktrees
        .ok_or_else(|| missing("provisioning.requested_worktrees"))?;
    let requested_worktrees = u32::try_from(requested)
        .ok()
        .filter(|value| (MIN_WORKTREES..=MAX_WORKTREES).contains(value))
        .ok_or_else(|| {
            invalid(
                "provisioning.requested_worktrees",
                format!("{requested} is outside {MIN_WORKTREES}-{MAX_WORKTREES}"),
            )
        })?;

    let dockerfile_sha256 = provisioning
        .dockerfile_sha256
        .ok_or_else(|| missing("provisioning.dockerfile_sha256"))?;
    require_sha256(&dockerfile_sha256)
        .map_err(|detail| invalid("provisioning.dockerfile_sha256", detail))?;

    let mut managed_worktrees = Vec::new();
    for (index, entry) in raw
        .worktrees
        .map(|worktrees| worktrees.managed)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        let worktree_path = entry
            .path
            .ok_or_else(|| missing("worktrees.managed.path"))?;
        require_worktree_name(&worktree_path).map_err(|detail| {
            invalid("worktrees.managed.path", format!("entry {index}: {detail}"))
        })?;
        let created_from = entry
            .created_from
            .ok_or_else(|| missing("worktrees.managed.created_from"))?;
        if git::branch_of_origin_ref(&created_from).is_none() {
            return Err(invalid(
                "worktrees.managed.created_from",
                format!("entry {index}: {created_from} is not a remote-tracking ref of origin"),
            ));
        }
        managed_worktrees.push(ManagedWorktree {
            path: worktree_path,
            created_from,
        });
    }

    let rebuild = match raw.rebuild {
        Some(rebuild) => {
            let target = rebuild
                .target_dockerfile_sha256
                .ok_or_else(|| missing("rebuild.target_dockerfile_sha256"))?;
            require_sha256(&target)
                .map_err(|detail| invalid("rebuild.target_dockerfile_sha256", detail))?;
            let previous = rebuild
                .previous_dockerfile_sha256
                .ok_or_else(|| missing("rebuild.previous_dockerfile_sha256"))?;
            require_sha256(&previous)
                .map_err(|detail| invalid("rebuild.previous_dockerfile_sha256", detail))?;
            Some(RebuildIntent {
                target_dockerfile_sha256: target,
                previous_dockerfile_sha256: previous,
            })
        }
        None => None,
    };

    Ok(ProjectMetadata {
        owner,
        repository,
        canonical_id,
        provisioning: Provisioning {
            mode,
            start_ref,
            requested_worktrees,
            dockerfile_sha256,
        },
        managed_worktrees,
        rebuild,
    })
}

/// SHA-256のlowercase hexであること。
fn require_sha256(value: &str) -> std::result::Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!("{value} is not a lowercase SHA-256 hex digest"));
    }
    Ok(())
}

/// managed worktreeのpathは、bare root直下の1階層の名前に限る。
fn require_worktree_name(value: &str) -> std::result::Result<(), String> {
    if value.is_empty() {
        return Err("the path is empty".to_string());
    }
    if Path::new(value).components().count() != 1 || value.contains('/') {
        return Err(format!(
            "{value} is not a single directory name below the bare repository"
        ));
    }
    if value == "." || value == ".." || value == ".git" {
        return Err(format!("{value} cannot be a managed worktree"));
    }
    Ok(())
}

/// metadataをTOMLへ描画する。
pub fn render(metadata: &ProjectMetadata) -> String {
    let mut out = String::new();
    out.push_str(&format!("version = {METADATA_VERSION}\n"));
    out.push_str(&format!("owner = {}\n", toml_string(&metadata.owner)));
    out.push_str(&format!(
        "repository = {}\n",
        toml_string(&metadata.repository)
    ));
    out.push_str(&format!(
        "canonical_id = {}\n",
        toml_string(metadata.canonical_id.as_str())
    ));

    let provisioning = &metadata.provisioning;
    out.push_str("\n[provisioning]\n");
    out.push_str(&format!(
        "mode = {}\n",
        toml_string(provisioning.mode.as_str())
    ));
    out.push_str(&format!(
        "start_ref = {}\n",
        toml_string(provisioning.start_ref.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "requested_worktrees = {}\n",
        provisioning.requested_worktrees
    ));
    out.push_str(&format!(
        "dockerfile_sha256 = {}\n",
        toml_string(&provisioning.dockerfile_sha256)
    ));

    for worktree in &metadata.managed_worktrees {
        out.push_str("\n[[worktrees.managed]]\n");
        out.push_str(&format!("path = {}\n", toml_string(&worktree.path)));
        out.push_str(&format!(
            "created_from = {}\n",
            toml_string(&worktree.created_from)
        ));
    }

    if let Some(rebuild) = &metadata.rebuild {
        out.push_str("\n[rebuild]\n");
        out.push_str(&format!(
            "target_dockerfile_sha256 = {}\n",
            toml_string(&rebuild.target_dockerfile_sha256)
        ));
        out.push_str(&format!(
            "previous_dockerfile_sha256 = {}\n",
            toml_string(&rebuild.previous_dockerfile_sha256)
        ));
    }

    out
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

/// `base_path`直下の案件metadataをすべて読む。
///
/// 対象は`<base-path>/*/*.project/.sbxm/project.toml`だけとし、directory entryと
/// metadata fileのsymlinkを追跡しない。1件の破損を無視して部分的な一覧を返さず、
/// 検出した不整合はすべて並べて返す。
pub fn discover(base: &AbsoluteBasePath) -> Result<Vec<DiscoveredProject>> {
    let mut found: Vec<DiscoveredProject> = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    for owner_dir in sorted_child_directories(base.as_path(), &mut diagnostics) {
        for project_root in sorted_child_directories(&owner_dir, &mut diagnostics) {
            if !project_root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(PROJECT_DIR_SUFFIX))
            {
                continue;
            }
            let metadata_path = project_root.join(".sbxm").join("project.toml");
            let text = match read_optional(&metadata_path) {
                Ok(Some(text)) => text,
                Ok(None) => continue,
                Err(error) => {
                    diagnostics.extend(error.diagnostics().iter().cloned());
                    continue;
                }
            };
            match parse(&text, &metadata_path) {
                Ok(metadata) => {
                    let paths = ProjectPaths::derive(base, &metadata.canonical_id);
                    if paths.root() != project_root {
                        // metadataが宣言する案件は、導出されるpathにだけ置ける。
                        diagnostics.push(Diagnostic::new(
                            ErrorId::MetadataPathMismatch,
                            msg!(
                                "error-metadata-path-mismatch",
                                path = paths::display(&metadata_path),
                                canonical_id = metadata.canonical_id,
                                expected = paths::display(paths.root())
                            ),
                        ));
                        continue;
                    }
                    found.push(DiscoveredProject { paths, metadata });
                }
                Err(error) => diagnostics.extend(error.diagnostics().iter().cloned()),
            }
        }
    }

    found.sort_by(|left, right| {
        left.metadata
            .canonical_id
            .as_str()
            .as_bytes()
            .cmp(right.metadata.canonical_id.as_str().as_bytes())
    });
    diagnostics.extend(conflicts(&found));

    if diagnostics.is_empty() {
        Ok(found)
    } else {
        Err(Error::Diagnostics(diagnostics))
    }
}

/// 衝突検査に使う、案件1件ぶんの識別子。
struct Registered {
    canonical_id: String,
    sandbox_name: String,
    root: String,
}

fn conflicts(found: &[DiscoveredProject]) -> Vec<Diagnostic> {
    let registered: Vec<Registered> = found
        .iter()
        .map(|project| Registered {
            canonical_id: project.metadata.canonical_id.to_string(),
            sandbox_name: project.metadata.sandbox_name().as_str().to_string(),
            root: paths::display(project.paths.root()),
        })
        .collect();
    conflicts_of(&registered)
}

/// canonical IDの重複と、Sandbox名の衝突。
///
/// 名前の対応だけを見るため、実際のhash値に依存せず検査できる。
fn conflicts_of(registered: &[Registered]) -> Vec<Diagnostic> {
    let mut by_canonical: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut by_sandbox: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for entry in registered {
        by_canonical
            .entry(&entry.canonical_id)
            .or_default()
            .push(&entry.root);
        by_sandbox
            .entry(&entry.sandbox_name)
            .or_default()
            .push(&entry.canonical_id);
    }

    let mut diagnostics = Vec::new();
    for (canonical, roots) in by_canonical {
        if roots.len() > 1 {
            diagnostics.push(Diagnostic::new(
                ErrorId::MetadataDuplicateProject,
                msg!(
                    "error-metadata-duplicate-project",
                    canonical_id = canonical,
                    paths = roots.join(", ")
                ),
            ));
        }
    }
    for (sandbox, mut canonical_ids) in by_sandbox {
        canonical_ids.dedup();
        if canonical_ids.len() > 1 {
            // hash prefixの理論上の衝突。安全側へ倒し、mutationしない。
            diagnostics.push(Diagnostic::new(
                ErrorId::SandboxNameCollision,
                msg!(
                    "error-sandbox-name-collision",
                    sandbox = sandbox,
                    projects = canonical_ids.join(", ")
                ),
            ));
        }
    }
    diagnostics
}

/// symlinkを追跡せずに、直下のdirectoryだけをbyte昇順で返す。
fn sorted_child_directories(parent: &Path, diagnostics: &mut Vec<Diagnostic>) -> Vec<PathBuf> {
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            diagnostics.push(Diagnostic::new(
                ErrorId::ProjectPathUnreadable,
                msg!(
                    "error-project-path-unreadable",
                    path = paths::display(parent),
                    detail = error
                ),
            ));
            return Vec::new();
        }
    };

    let mut directories = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        // symlinkは追跡しない。指す先は案件directoryの外にあり得る。
        if metadata.is_dir() {
            directories.push(path);
        }
    }
    directories.sort();
    directories
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn canonical(value: &str) -> CanonicalProjectId {
        ProjectId::parse(value)
            .expect("valid project id")
            .canonical()
    }

    const DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const OTHER_DIGEST: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn attached(owner: &str, repository: &str) -> ProjectMetadata {
        ProjectMetadata {
            owner: owner.to_string(),
            repository: repository.to_string(),
            canonical_id: canonical(&format!("{owner}/{repository}")),
            provisioning: Provisioning {
                mode: CreationMode::Attached,
                start_ref: Some("main".to_string()),
                requested_worktrees: 1,
                dockerfile_sha256: DIGEST.to_string(),
            },
            managed_worktrees: vec![ManagedWorktree {
                path: format!("{}.tree-0", repository.to_ascii_lowercase()),
                created_from: git::origin_ref("main"),
            }],
            rebuild: None,
        }
    }

    fn round_trip(metadata: &ProjectMetadata) -> ProjectMetadata {
        parse(&render(metadata), Path::new("/tmp/project.toml")).expect("the rendered form parses")
    }

    fn write_project(base: &Path, owner: &str, repository: &str, text: &str) -> PathBuf {
        let root = base
            .join(owner.to_ascii_lowercase())
            .join(format!("{}.project", repository.to_ascii_lowercase()));
        let dir = root.join(".sbxm");
        fs::create_dir_all(&dir).expect("create .sbxm");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("mode");
        let path = dir.join("project.toml");
        fs::write(&path, text).expect("write metadata");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode");
        root
    }

    #[test]
    fn metadata_round_trips_through_the_rendered_form() {
        let metadata = attached("Example-Org", "Example-Repo");
        assert_eq!(round_trip(&metadata), metadata);

        let detached = ProjectMetadata {
            provisioning: Provisioning {
                mode: CreationMode::Detached,
                start_ref: Some("develop".to_string()),
                requested_worktrees: 3,
                dockerfile_sha256: DIGEST.to_string(),
            },
            managed_worktrees: (0..3)
                .map(|index| ManagedWorktree {
                    path: format!("example-repo.tree-{index}"),
                    created_from: git::origin_ref("develop"),
                })
                .collect(),
            rebuild: Some(RebuildIntent {
                target_dockerfile_sha256: OTHER_DIGEST.to_string(),
                previous_dockerfile_sha256: DIGEST.to_string(),
            }),
            ..attached("example-org", "example-repo")
        };
        assert_eq!(round_trip(&detached), detached);
    }

    #[test]
    fn an_attached_project_may_wait_for_the_remote_default_branch() {
        let mut metadata = attached("example-org", "example-repo");
        metadata.provisioning.start_ref = None;
        metadata.managed_worktrees.clear();
        assert_eq!(round_trip(&metadata), metadata);

        // detached modeは起点branchの明示を必須とする。
        let text = render(&metadata).replace("attached", "detached");
        let error = parse(&text, Path::new("/tmp/project.toml"))
            .expect_err("a detached project needs a start branch");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
    }

    #[test]
    fn an_unknown_version_is_diagnosed_before_other_fields() {
        let text = "version = 2\n";
        let error = parse(text, Path::new("/tmp/project.toml")).expect_err("unknown versions fail");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataUnknownVersion));
    }

    #[test]
    fn required_fields_are_named_when_they_are_missing() {
        let full = render(&attached("example-org", "example-repo"));
        for field in [
            "owner = ",
            "repository = ",
            "canonical_id = ",
            "mode = ",
            "start_ref = ",
            "requested_worktrees = ",
            "dockerfile_sha256 = ",
        ] {
            let text: String = full
                .lines()
                .filter(|line| !line.starts_with(field))
                .map(|line| format!("{line}\n"))
                .collect();
            let error =
                parse(&text, Path::new("/tmp/project.toml")).expect_err("{field} is required");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::MetadataMissingField),
                "{field} produced the wrong error"
            );
        }
    }

    #[test]
    fn the_canonical_identifier_must_match_the_display_names() {
        let text = render(&attached("Example-Org", "Example-Repo"))
            .replace("example-org/example-repo", "other-org/other-repo");
        let error = parse(&text, Path::new("/tmp/project.toml"))
            .expect_err("the canonical ID must fold from owner and repository");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));

        let text = render(&attached("example-org", "example-repo")).replace(
            "canonical_id = \"example-org",
            "canonical_id = \"Example-Org",
        );
        let error = parse(&text, Path::new("/tmp/project.toml"))
            .expect_err("the canonical ID is always folded");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
    }

    #[test]
    fn values_outside_the_documented_range_are_refused() {
        let base = render(&attached("example-org", "example-repo"));
        for (from, to) in [
            ("requested_worktrees = 1", "requested_worktrees = 0"),
            ("requested_worktrees = 1", "requested_worktrees = 33"),
            ("mode = \"attached\"", "mode = \"half-attached\""),
            ("dockerfile_sha256 = \"1111", "dockerfile_sha256 = \"NOTHEX"),
            (
                "created_from = \"refs/remotes/origin/main\"",
                "created_from = \"refs/heads/main\"",
            ),
            ("path = \"example-repo.tree-0\"", "path = \"../escape\""),
        ] {
            let text = base.replace(from, to);
            assert_ne!(text, base, "the replacement {from} did not apply");
            let error =
                parse(&text, Path::new("/tmp/project.toml")).expect_err("{to} must be refused");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::MetadataInvalidValue),
                "{to} produced the wrong error"
            );
        }
    }

    #[test]
    fn metadata_is_written_privately_and_replaced_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        let metadata = attached("example-org", "example-repo");
        let project = ProjectPaths::derive(&base, &metadata.canonical_id);
        fs::create_dir_all(project.sbxm_dir()).unwrap();

        create(&project, &metadata).expect("create");
        let mode = fs::metadata(project.metadata_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(load(&project).unwrap().as_ref(), Some(&metadata));

        let mut resolved = metadata.clone();
        resolved.provisioning.start_ref = Some("develop".to_string());
        update(&project, &resolved).expect("update");
        assert_eq!(load(&project).unwrap(), Some(resolved));
    }

    #[test]
    fn a_missing_metadata_file_is_not_an_error_but_a_symlinked_one_is() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo"));
        assert_eq!(load(&project).unwrap(), None);

        fs::create_dir_all(project.sbxm_dir()).unwrap();
        let real = dir.path().join("elsewhere.toml");
        fs::write(&real, render(&attached("example-org", "example-repo"))).unwrap();
        std::os::unix::fs::symlink(&real, project.metadata_file()).unwrap();

        let error = load(&project).expect_err("a symlinked metadata file is refused");
        assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
    }

    #[test]
    fn discovery_returns_every_project_in_canonical_order() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        for (owner, repository) in [("zeta", "repo"), ("alpha", "repo"), ("alpha", "another")] {
            write_project(
                dir.path(),
                owner,
                repository,
                &render(&attached(owner, repository)),
            );
        }

        let found = discover(&base).expect("every project parses");
        let ids: Vec<String> = found
            .iter()
            .map(|project| project.metadata.canonical_id.to_string())
            .collect();
        assert_eq!(ids, vec!["alpha/another", "alpha/repo", "zeta/repo"]);
    }

    #[test]
    fn discovery_looks_at_the_documented_shape_only() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        write_project(
            dir.path(),
            "example-org",
            "example-repo",
            &render(&attached("example-org", "example-repo")),
        );
        // 深すぎるpath、`.project`で終わらないdirectory、metadataのないdirectory。
        fs::create_dir_all(dir.path().join("owner/deeper/nested.project/.sbxm")).unwrap();
        fs::write(
            dir.path()
                .join("owner/deeper/nested.project/.sbxm/project.toml"),
            render(&attached("owner", "nested")),
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("owner/plain-directory")).unwrap();

        let found = discover(&base).expect("only the documented shape is read");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].metadata.canonical_id.to_string(),
            "example-org/example-repo"
        );
    }

    #[test]
    fn a_symlinked_project_directory_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        let real = write_project(
            dir.path(),
            "example-org",
            "example-repo",
            &render(&attached("example-org", "example-repo")),
        );
        fs::create_dir_all(dir.path().join("linked-org")).unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("linked-org/example-repo.project"))
            .unwrap();

        let found = discover(&base).expect("the symlink is skipped rather than followed");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].paths.root(), real);
    }

    #[test]
    fn one_broken_project_stops_the_whole_listing() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        write_project(
            dir.path(),
            "example-org",
            "example-repo",
            &render(&attached("example-org", "example-repo")),
        );
        write_project(dir.path(), "broken-org", "broken-repo", "version = 2\n");

        let error = discover(&base).expect_err("a partial listing is never returned");
        assert!(error.contains(ErrorId::MetadataUnknownVersion), "{error:?}");
    }

    #[test]
    fn a_project_stored_outside_its_derived_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        // metadataはexample-orgの案件だが、別のowner directoryへ置かれている。
        write_project(
            dir.path(),
            "other-org",
            "example-repo",
            &render(&attached("example-org", "example-repo")),
        );

        let error =
            discover(&base).expect_err("the derived path is the only place a project lives");
        assert!(error.contains(ErrorId::MetadataPathMismatch), "{error:?}");
    }

    fn registered(canonical_id: &str, sandbox_name: &str, root: &str) -> Registered {
        Registered {
            canonical_id: canonical_id.to_string(),
            sandbox_name: sandbox_name.to_string(),
            root: root.to_string(),
        }
    }

    #[test]
    fn distinct_projects_are_not_a_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let base = AbsoluteBasePath::new(dir.path()).unwrap();
        let projects: Vec<DiscoveredProject> = [("example-org", "example-repo"), ("other", "repo")]
            .into_iter()
            .map(|(owner, repository)| {
                let metadata = attached(owner, repository);
                DiscoveredProject {
                    paths: ProjectPaths::derive(&base, &metadata.canonical_id),
                    metadata,
                }
            })
            .collect();
        assert!(conflicts(&projects).is_empty());
    }

    #[test]
    fn two_directories_claiming_the_same_project_are_listed_as_a_conflict() {
        let diagnostics = conflicts_of(&[
            registered("example-org/example-repo", "sbxm-a", "/base/a.project"),
            registered("example-org/example-repo", "sbxm-a", "/base/b.project"),
        ]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, ErrorId::MetadataDuplicateProject);
    }

    #[test]
    fn two_projects_that_derive_the_same_sandbox_name_are_listed_as_a_conflict() {
        // hash prefixの衝突は実際には起こせないため、導出結果の対応だけを検査する。
        let diagnostics = conflicts_of(&[
            registered("example-org/example-repo", "sbxm-shared", "/base/a.project"),
            registered("other-org/other-repo", "sbxm-shared", "/base/b.project"),
        ]);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].id, ErrorId::SandboxNameCollision);
    }
}
