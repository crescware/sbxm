use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::config::GlobalConfig;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxName};

use crate::design::ProgressSink;
use crate::design::Remediation;
use crate::project::SandboxLayout;
use crate::support::files::{self};
use crate::support::{daemon, generation, inventory, repository, sandbox, select};

use super::{ApplyOutput, Scope, Target};

/// 対象を引数またはpromptで解決し、構築済みの案件へ変更を適用する。
///
/// Sandboxの中身を変えるmutationであるため、対象を確かめた後にproject lockを取得し、
/// lock取得後のmetadataでpreconditionを判定し直してから適用する。
pub fn run(
    target: Target,
    config: &GlobalConfig,
    scope: Scope,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ApplyOutput> {
    let Target {
        location,
        requested,
        prompt,
    } = target;
    // 対象が決まる前にhostの状態へ触れない。
    let mut locked =
        select::one(location, requested, &msg!("select-apply-heading"), prompt)?.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;

    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let entry = daemon::list(host)?
        .into_iter()
        .find(|entry| entry.name == name.as_str())
        .ok_or_else(|| inventory::not_created(&locked.metadata, name.as_str()))?;

    sandbox::verify_identity(&entry, &name, workspace_root)?;

    if entry.state != SandboxState::Running {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotRunning,
                msg!(
                    "error-sandbox-not-running",
                    sandbox = entry.name,
                    observed = entry.state.as_str()
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-sandbox-not-running"))
                    .try_run(format!("sbxm open {}", locked.metadata.display_id())),
            ),
        ));
    }

    let mut files = Vec::new();
    if scope.files {
        files = files::place_all(host, &entry.name, &config.files, files::Conflict::Overwrite)?;
    }

    let mut worktrees = None;
    if let Some(count) = scope.worktrees {
        raise_worktrees(&locked.paths, &mut locked.metadata, count)?;
        let layout = SandboxLayout::new(&canonical);
        let project = ProjectId::parse(&locked.metadata.display_id())?;
        repository::ensure_bare_clone(host, &entry.name, &project, &layout, progress)?;
        let branch = repository::resolve_start_ref(
            host,
            &entry.name,
            &layout,
            &locked.paths,
            &mut locked.metadata,
        )?;
        repository::ensure_worktrees(
            host,
            &entry.name,
            &layout,
            &locked.metadata,
            &branch,
            progress,
        )?;
        worktrees = Some(locked.metadata.provisioning.requested_worktrees);
    }

    Ok(ApplyOutput {
        project: locked.metadata.display_id(),
        sandbox: entry.name,
        files,
        worktrees,
    })
}

/// 目標worktree数を引き上げる。
///
/// 減らす指定は受け付けない。worktreeを減らすことはcheckoutされた作業を消すことであり、
/// `destroy`と同じ重さの確認が要る。
fn raise_worktrees(paths: &ProjectPaths, metadata: &mut ProjectMetadata, count: u32) -> Result<()> {
    let current = metadata.provisioning.requested_worktrees;
    if count < current {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::WorktreesNotReducible,
                msg!(
                    "error-worktrees-not-reducible",
                    project = metadata.display_id(),
                    requested = count,
                    current = current
                ),
            )
            .remediation(msg!("remediation-worktrees-not-reducible")),
        ));
    }
    if count == current {
        return Ok(());
    }
    metadata.provisioning.requested_worktrees = count;
    metadata::update(paths, metadata)
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
