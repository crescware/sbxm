//! `sbxm apply`。
//!
//! 構築済みの案件へ、作り直さずに反映できる変更を適用する。適用するものはoptionで
//! 明示させる。省略した対象には触れない。
//!
//! 作り直しを要する変更は`rebuild`が担当する。projectの登録、構築の継続、
//! image・Template操作は行わない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxName};

use super::files::{self, PlacedFile};
use super::tools::Note;
use super::{daemon, inventory, rebuild, repository, sandbox, select, tools};
use crate::project::SandboxLayout;

/// 何を適用するか。
///
/// 省略した対象は変更しない。宣言fileの配置は既存のfileを上書きするため、暗黙には
/// 走らせない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope {
    /// global configが宣言するfileを再配置する。
    pub files: bool,
    /// managed worktreeの目標本数。現在より多い値だけを受け付ける。
    pub worktrees: Option<u32>,
}

/// `apply`の結果。
#[derive(Debug, Clone)]
pub struct ApplyOutput {
    pub project: String,
    pub sandbox: String,
    pub files: Vec<PlacedFile>,
    /// worktreeを適用した場合の、適用後の本数。
    pub worktrees: Option<u32>,
    /// Sandboxに入っているtoolが返した案内。
    pub notes: Vec<Note>,
}

/// 構築済みの案件へ変更を適用する。
///
/// Sandboxの中身を変えるmutationであるため、対象を確かめた後にproject lockを取得し、
/// lock取得後のmetadataでpreconditionを判定し直してから適用する。
pub fn run(
    config: &GlobalConfig,
    project: &ProjectId,
    scope: Scope,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<ApplyOutput> {
    let canonical = project.canonical();
    let mut locked = select::locked(config, project)?;
    rebuild::require_no_rebuild(&locked.metadata)?;

    let name = SandboxName::derive(&canonical);
    let entry = daemon::list(host)?
        .into_iter()
        .find(|entry| entry.name == name.as_str())
        .ok_or_else(|| inventory::not_created(&locked.metadata, name.as_str()))?;

    sandbox::verify_identity(&entry, &name, workspace_root)?;

    if entry.state != SandboxState::Running {
        // 停止中のSandboxを暗黙に起動しない。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotRunning,
                msg!(
                    "error-sandbox-not-running",
                    sandbox = entry.name,
                    observed = entry.state.as_str()
                ),
            )
            .remediation(msg!(
                "remediation-sandbox-not-running",
                command = format!("sbxm open {}", locked.metadata.display_id())
            )),
        ));
    }

    let mut files = Vec::new();
    if scope.files {
        files = files::place_all(host, &entry.name, &config.files, files::Conflict::Overwrite)?;
    }

    let mut worktrees = None;
    let mut notes = Vec::new();
    if let Some(count) = scope.worktrees {
        raise_worktrees(&locked.paths, &mut locked.metadata, count)?;
        let layout = SandboxLayout::new(&canonical);
        repository::ensure_bare_clone(host, &entry.name, project, &layout)?;
        let branch = repository::resolve_start_ref(
            host,
            &entry.name,
            &layout,
            &locked.paths,
            &mut locked.metadata,
        )?;
        let managed =
            repository::ensure_worktrees(host, &entry.name, &layout, &locked.metadata, &branch)?;
        worktrees = Some(locked.metadata.provisioning.requested_worktrees);
        notes = tools::worktrees_ready(host, &entry.name, &layout, managed.len())?;
    }

    Ok(ApplyOutput {
        project: locked.metadata.display_id(),
        sandbox: entry.name,
        files,
        worktrees,
        notes,
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
#[path = "apply_test.rs"]
mod apply_test;
