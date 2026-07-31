//! Global registry `~/.sbxm/registry.yaml`。
//!
//! 任意の場所へ配置された全案件を、単一のdocumentで索引する。registryが持つのは索引と
//! 不変なrepository identityだけであり、可変な目標構成やGit identityはproject rootの
//! `.sbxm/project.yaml`だけを正本とする。
//!
//! 登録状態を表すfieldを保存しない。登録がどこまで進んでいるかは、entry、project root、
//! metadata、host cloneというfilesystem上の事実を観測して算出する。
//!
//! mutationは`~/.sbxm/registry.lock`に対するglobal exclusive lockで直列化し、documentを
//! 単純追記しない。全entryを検証し、memory上で完全なdocumentを組み立ててからatomicに
//! 置き換える。一部entryだけが正常でも、壊れたregistryをmutationの根拠として信用しない。

mod document;
mod parse;
mod render;

pub use parse::parse;
pub use render::render;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::config::{self, ConfigLocation};
use crate::error::{Diagnostic, DocumentVersion, Error, ErrorId, Result, fail};
use crate::msg;
use crate::paths::{
    self, ExclusiveLock, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, acquire_exclusive_lock,
    atomic_create, atomic_replace, permission_too_open,
};
use crate::project::{CanonicalProjectId, SandboxName};
use crate::repository::RepositoryIdentity;

/// このbuildが読み書きするregistryのversion。
pub const REGISTRY_VERSION: u32 = 1;

/// registryのversionの読み方。
const DOCUMENT: DocumentVersion = DocumentVersion {
    supported: REGISTRY_VERSION,
    unknown: ErrorId::RegistryUnknownVersion,
    unknown_message: "error-registry-unknown-version",
};

/// registryの1 entry。
///
/// 登録意図そのものであり、project rootやmetadataがまだ無い状態も有効なentryとする。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    project_root: PathBuf,
    repository: RepositoryIdentity,
}

impl RegistryEntry {
    /// validation済みのproject rootとrepository identityからentryを作る。
    pub fn new(project_root: &Path, repository: RepositoryIdentity) -> Result<RegistryEntry> {
        Ok(RegistryEntry {
            project_root: require_absolute_root(project_root)?,
            repository,
        })
    }

    /// 突き合わせの正本。
    pub fn canonical_id(&self) -> &CanonicalProjectId {
        self.repository.canonical_id()
    }

    /// 保存済みの絶対project root。
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn repository(&self) -> &RepositoryIdentity {
        &self.repository
    }

    /// canonical project `IDから決定的に導出したSandbox名`。
    pub fn sandbox_name(&self) -> SandboxName {
        SandboxName::derive(self.canonical_id())
    }
}

/// 検証済みのregistry document全体。
///
/// 不在のregistryは0件として扱う。0件と読めなかったregistryを同じ値にしない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    entries: Vec<RegistryEntry>,
}

impl Registry {
    /// canonical project `ID昇順のentry`。
    pub fn entries(&self) -> &[RegistryEntry] {
        &self.entries
    }

    pub fn find(&self, canonical: &CanonicalProjectId) -> Option<&RegistryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.canonical_id() == canonical)
    }

    /// 全entryを突き合わせ、registryの不変条件を検査する。
    fn check_invariants(&self) -> Result<()> {
        let mut diagnostics = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            for other in &self.entries[index + 1..] {
                if entry.canonical_id() == other.canonical_id() {
                    diagnostics.push(Diagnostic::new(
                        ErrorId::RegistryDuplicateProject,
                        msg!(
                            "error-registry-duplicate-project",
                            canonical_id = entry.canonical_id(),
                            paths = format!(
                                "{}, {}",
                                paths::display(entry.project_root()),
                                paths::display(other.project_root())
                            )
                        ),
                    ));
                    continue;
                }
                if entry.project_root() == other.project_root() {
                    diagnostics.push(Diagnostic::new(
                        ErrorId::RegistryDuplicateRoot,
                        msg!(
                            "error-registry-duplicate-root",
                            path = paths::display(entry.project_root()),
                            projects =
                                format!("{}, {}", entry.canonical_id(), other.canonical_id())
                        ),
                    ));
                    continue;
                }
                if entry.sandbox_name() == other.sandbox_name() {
                    // hash prefixの理論上の衝突。安全側へ倒し、mutationしない。
                    diagnostics.push(Diagnostic::new(
                        ErrorId::SandboxNameCollision,
                        msg!(
                            "error-sandbox-name-collision",
                            sandbox = entry.sandbox_name(),
                            projects =
                                format!("{}, {}", entry.canonical_id(), other.canonical_id())
                        ),
                    ));
                }
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(Error::Diagnostics(diagnostics))
        }
    }

    fn sort(&mut self) {
        self.entries.sort_by(|left, right| {
            left.canonical_id()
                .as_str()
                .as_bytes()
                .cmp(right.canonical_id().as_str().as_bytes())
        });
    }
}

/// registryをread-onlyで読む。
///
/// fileが存在しないことは、登録案件0件として正常に扱う。読めたが不正なregistryを
/// 0件へ丸めない。
pub fn load(location: &ConfigLocation) -> Result<Registry> {
    let path = location.registry_file();

    if paths::is_symlink(&path) {
        return Err(PathScope::ConfigFile.symlink_error(&path));
    }

    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Registry::default());
        }
        Err(error) => return Err(unreadable(&path, &error.to_string())),
    };
    if !metadata.is_file() {
        return Err(unreadable(&path, "the registry path is not a regular file"));
    }
    let mode = metadata.permissions().mode();
    if permission_too_open(mode) {
        return Err(PathScope::ConfigFile.permission_error(&path, mode, PRIVATE_FILE_MODE));
    }

    let text = fs::read_to_string(&path).map_err(|error| unreadable(&path, &error.to_string()))?;
    let registry = parse(&text, &path)?;
    registry.check_invariants()?;
    Ok(registry)
}

/// registry lockを保持したままregistryを読み書きする区間。
///
/// 値が生きているあいだ、registryへのmutationは全processで直列化される。複数lockが
/// 必要なworkflowでは、常にregistry lock、project lockの順で取得する。
#[derive(Debug)]
pub struct RegistryGuard {
    path: PathBuf,
    registry: Registry,
    _lock: ExclusiveLock,
}

impl RegistryGuard {
    /// `~/.sbxm`を用意し、registry lockを取ってからdocument全体を読む。
    pub fn acquire(location: &ConfigLocation) -> Result<RegistryGuard> {
        config::ensure_config_dir(location)?;
        let lock = acquire_exclusive_lock(
            &location.registry_lock(),
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ConfigFile,
        )?;
        Ok(RegistryGuard {
            path: location.registry_file(),
            registry: load(location)?,
            _lock: lock,
        })
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// entryを追加し、documentを丸ごと書き直す。
    ///
    /// 既に同じcanonical project `IDのentryがある場合は何も書かない`。
    pub fn insert(&mut self, entry: RegistryEntry) -> Result<()> {
        if let Some(existing) = self.registry.find(entry.canonical_id()) {
            if existing == &entry {
                return Ok(());
            }
            return Err(conflicting_entry(existing, &entry));
        }
        let mut updated = self.registry.clone();
        updated.entries.push(entry);
        updated.sort();
        updated.check_invariants()?;
        self.write(updated)
    }

    /// entryを削除し、documentを丸ごと書き直す。
    ///
    /// 対象がなければ何も書かない。
    pub fn remove(&mut self, canonical: &CanonicalProjectId) -> Result<()> {
        if self.registry.find(canonical).is_none() {
            return Ok(());
        }
        let mut updated = self.registry.clone();
        updated
            .entries
            .retain(|entry| entry.canonical_id() != canonical);
        self.write(updated)
    }

    fn write(&mut self, updated: Registry) -> Result<()> {
        let text = render(&updated)?;
        if paths::regular_file_exists(&self.path, PathScope::ConfigFile)? {
            atomic_replace(&self.path, &text, PRIVATE_FILE_MODE)?;
        } else {
            atomic_create(&self.path, &text, PRIVATE_FILE_MODE)?;
        }
        self.registry = updated;
        Ok(())
    }
}

/// project rootとして保存してよいpathか。
fn require_absolute_root(declared: &Path) -> Result<PathBuf> {
    if !declared.is_absolute() {
        return fail(
            ErrorId::RegistryInvalidValue,
            msg!(
                "error-registry-invalid-value",
                field = "project_root",
                detail = format!("{} is not an absolute path", paths::display(declared))
            ),
        );
    }
    let standardized = paths::lexically_standardize(declared);
    if standardized != declared {
        return fail(
            ErrorId::RegistryInvalidValue,
            msg!(
                "error-registry-invalid-value",
                field = "project_root",
                detail = format!(
                    "{} holds a relative component and resolves to {}",
                    paths::display(declared),
                    paths::display(&standardized)
                )
            ),
        );
    }
    Ok(standardized)
}

/// 同じcanonical project IDに対して、別の登録内容が既にある。
fn conflicting_entry(existing: &RegistryEntry, requested: &RegistryEntry) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::RegistryEntryMismatch,
            msg!(
                "error-registry-entry-mismatch",
                canonical_id = existing.canonical_id(),
                stored = format!(
                    "{} {}",
                    paths::display(existing.project_root()),
                    existing.repository().clone_url()
                ),
                requested = format!(
                    "{} {}",
                    paths::display(requested.project_root()),
                    requested.repository().clone_url()
                )
            ),
        )
        .remediation(msg!("remediation-registry-entry-mismatch")),
    )
}

fn unreadable(path: &Path, detail: &str) -> Error {
    Error::new(
        ErrorId::RegistryUnreadable,
        msg!(
            "error-registry-unreadable",
            path = paths::display(path),
            detail = detail
        ),
    )
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
