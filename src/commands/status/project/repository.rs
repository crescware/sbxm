//! bare repositoryとmanaged worktreeの診断。

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::{sandbox, worktree};

use super::{ProjectStatus, Value, WorktreeRow};

pub(super) fn check_bare_repository(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    status: &mut ProjectStatus,
) {
    let git_dir = layout.bare_git_dir();
    let outcome = sandbox::exec(
        host,
        name.as_str(),
        &[
            "git",
            "--git-dir",
            &git_dir,
            "rev-parse",
            "--is-bare-repository",
        ],
    );
    let value = match outcome {
        Ok(outcome) => match sandbox::inner_exit_code(&outcome) {
            Some(0) if outcome.stdout_text().trim() == "true" => Value::Ready,
            Some(0) => {
                status.diagnostics.push(Diagnostic::new(
                    ErrorId::SandboxRepositoryUnusable,
                    msg!(
                        "error-sandbox-repository-unusable",
                        path = git_dir,
                        detail = "the shared repository is not bare"
                    ),
                ));
                Value::Mismatch
            }
            // `git`がrepositoryとして扱えない場合の終了statusだけを不在とする。
            Some(sandbox::GIT_FATAL) => Value::Missing,
            _ => {
                status.diagnostics.extend(
                    sandbox::unobservable(&outcome, &git_dir)
                        .diagnostics()
                        .iter()
                        .cloned(),
                );
                Value::Mismatch
            }
        },
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-bare-repository", value);
}

/// Sandbox内のworktreeを、metadataと突き合わせて分類する。
pub(super) fn check_worktrees(
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

/// 作業中の変更があるか。submoduleの変更も`git status`が示すとおりに扱う。
pub(super) fn worktree_state(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    path: &str,
    status: &mut ProjectStatus,
) -> Value {
    match sandbox::exec(
        host,
        name.as_str(),
        &[
            "git",
            "-C",
            path,
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
        ],
    ) {
        Ok(outcome) if outcome.success() => {
            if outcome
                .stdout_text()
                .trim_matches(['\0', '\n', ' '])
                .is_empty()
            {
                Value::Clean
            } else {
                Value::Dirty
            }
        }
        Ok(outcome) => {
            status.diagnostics.extend(
                sandbox::unobservable(&outcome, path)
                    .diagnostics()
                    .iter()
                    .cloned(),
            );
            Value::Mismatch
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    }
}
