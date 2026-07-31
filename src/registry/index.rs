use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self};
use crate::project::CanonicalProjectId;

use super::RegistryEntry;

/// 検証済みのregistry document全体。
///
/// 不在のregistryは0件として扱う。0件と読めなかったregistryを同じ値にしない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    pub(super) entries: Vec<RegistryEntry>,
}

impl Index {
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
    pub(crate) fn check_invariants(&self) -> Result<()> {
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

    pub(crate) fn sort(&mut self) {
        self.entries.sort_by(|left, right| {
            left.canonical_id()
                .as_str()
                .as_bytes()
                .cmp(right.canonical_id().as_str().as_bytes())
        });
    }
}
