use crate::command::HostEnvironment;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::worktree;

use crate::commands::status::project::{ProjectStatus, Value, WorktreeRow};

use super::worktree_state;

/// Sandbox内のworktreeを、metadataと突き合わせて分類する。
pub fn check_worktrees(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let entries = match worktree::list(host, name.as_str(), layout) {
        Ok(entries) => entries,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            status.push("status-item-worktrees", Value::Mismatch);
            return;
        }
    };

    let bare_root = layout.bare_root();
    let declared = layout.worktree_names(metadata.provisioning.requested_worktrees);
    let mut seen: Vec<String> = Vec::new();
    let mut value = Value::Ready;
    for entry in entries {
        if entry.bare {
            continue;
        }
        let Some(relative) = entry.relative_to(&bare_root) else {
            status.diagnostics.push(Diagnostic::new(
                ErrorId::SandboxRepositoryUnusable,
                msg!(
                    "error-sandbox-repository-unusable",
                    path = entry.path,
                    detail = "the worktree is outside the shared repository"
                ),
            ));
            value = Value::Mismatch;
            continue;
        };
        let managed = declared.contains(&relative);
        seen.push(relative.clone());

        let mode = if entry.detached {
            Value::Detached
        } else {
            Value::Attached
        };
        let state = worktree_state(host, name, &entry.path, status);
        if state == Value::Mismatch {
            value = Value::Mismatch;
        }
        status.worktrees.push(WorktreeRow {
            path: relative,
            kind: if managed { "managed" } else { "unmanaged" },
            mode,
            state,
        });
    }

    for name in &declared {
        if !seen.contains(name) {
            status.diagnostics.push(Diagnostic::new(
                ErrorId::SandboxRepositoryUnusable,
                msg!(
                    "error-sandbox-repository-unusable",
                    path = name,
                    detail = "the project asks for this managed worktree, but Git does not have it"
                ),
            ));
            status.worktrees.push(WorktreeRow {
                path: name.clone(),
                kind: "managed",
                mode: Value::Mismatch,
                state: Value::Mismatch,
            });
            value = Value::Mismatch;
        }
    }
    status.push("status-item-worktrees", value);
}
