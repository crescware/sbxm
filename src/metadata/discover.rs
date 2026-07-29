//! base path配下のmetadata探索と、登録の衝突。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, AbsoluteBasePath, PROJECT_DIR_SUFFIX, ProjectPaths};

use super::parse::parse;
use super::{DiscoveredProject, read_optional};

/// `base_path`直下の案件metadataをすべて読む。
///
/// 対象は`<base-path>/*/*.project/.sbxm/project.yaml`だけとし、directory entryと
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
            let metadata_path = project_root.join(".sbxm").join("project.yaml");
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
pub(super) struct Registered {
    canonical_id: String,
    sandbox_name: String,
    root: String,
}

pub(super) fn conflicts(found: &[DiscoveredProject]) -> Vec<Diagnostic> {
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
pub(super) fn conflicts_of(registered: &[Registered]) -> Vec<Diagnostic> {
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
pub(super) fn sorted_child_directories(
    parent: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<PathBuf> {
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
#[path = "discover_test.rs"]
mod discover_test;
