use std::path::PathBuf;

use crate::config::{self, ConfigLocation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{
    self, ExclusiveLock, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, acquire_exclusive_lock,
    atomic_create, atomic_replace,
};
use crate::project::CanonicalProjectId;

use super::{Index, RegistryEntry, load, render};

/// registry lockを保持したままregistryを読み書きする区間。
///
/// 値が生きているあいだ、registryへのmutationは全processで直列化される。複数lockが
/// 必要なworkflowでは、常にregistry lock、project lockの順で取得する。
#[derive(Debug)]
pub struct RegistryGuard {
    path: PathBuf,
    registry: Index,
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

    pub fn registry(&self) -> &Index {
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

    fn write(&mut self, updated: Index) -> Result<()> {
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
