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
use crate::paths::{self, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::project::{ProjectId, SandboxName};

use super::files::{self, PlacedFile};
use super::tools::Note;
use super::{daemon, repository, sandbox, tools};
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
    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    let not_managed = || {
        Error::single(
            Diagnostic::new(
                ErrorId::ProjectNotManaged,
                msg!("error-project-not-managed", project = project),
            )
            .remediation(msg!(
                "remediation-project-not-managed",
                command = format!("sbxm add {project}")
            )),
        )
    };

    if metadata::load(&paths)?.is_none() {
        // 管理対象でない案件にはlock fileも作らない。
        return Err(not_managed());
    }
    let _lock = paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )?;

    // lock取得後のmetadataを、以降の判定の正本とする。
    let Some(mut metadata) = metadata::load(&paths)? else {
        return Err(not_managed());
    };
    require_no_rebuild(&metadata)?;

    let name = SandboxName::derive(&canonical);
    let entry = daemon::list(host)?
        .into_iter()
        .find(|entry| entry.name == name.as_str())
        .ok_or_else(|| {
            Error::single(
                Diagnostic::new(
                    ErrorId::SandboxNotCreated,
                    msg!(
                        "error-sandbox-not-created",
                        project = metadata.display_id(),
                        sandbox = name
                    ),
                )
                .remediation(msg!(
                    "remediation-sandbox-not-created",
                    command = format!("sbxm add {}", metadata.display_id())
                )),
            )
        })?;

    sandbox::verify_identity(&entry, &name, workspace_root)?;

    if entry.state != SandboxState::Running {
        // 停止中のSandboxを暗黙に起動しない。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotRunning,
                msg!(
                    "error-sandbox-not-running",
                    sandbox = entry.name,
                    observed = state_of(entry.state)
                ),
            )
            .remediation(msg!(
                "remediation-sandbox-not-running",
                command = format!("sbxm open {}", metadata.display_id())
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
        raise_worktrees(&paths, &mut metadata, count)?;
        let layout = SandboxLayout::new(&canonical);
        repository::ensure_bare_clone(host, &entry.name, project, &layout)?;
        let branch =
            repository::resolve_start_ref(host, &entry.name, &layout, &paths, &mut metadata)?;
        let managed = repository::ensure_worktrees(host, &entry.name, &layout, &metadata, &branch)?;
        worktrees = Some(metadata.provisioning.requested_worktrees);
        notes = tools::worktrees_ready(host, &entry.name, &layout, managed.len())?;
    }

    Ok(ApplyOutput {
        project: metadata.display_id(),
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

/// 世代の切替中は、fileを配置せず`rebuild`の再実行を案内する。
fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(msg!(
            "remediation-run-rebuild",
            command = format!("sbxm rebuild {}", metadata.display_id())
        )),
    ))
}

/// 翻訳しない状態値。
fn state_of(state: SandboxState) -> &'static str {
    match state {
        SandboxState::Running => "running",
        SandboxState::Stopped => "stopped",
    }
}

#[cfg(test)]
#[path = "apply_test.rs"]
mod apply_test;
