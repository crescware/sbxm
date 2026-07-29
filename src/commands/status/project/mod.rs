//! `sbxm status <owner>/<repository>`。
//!
//! 1案件の構築状態、作業可能性、credential隔離をread-onlyで診断する。repair、起動、
//! 停止、file更新を行わない。作成元やsbxm独自のmarkerは検査せず、現在の状態だけを見る。

#[cfg(test)]
mod fake;

mod artifacts;
mod inside;
mod repository;

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxName};

use crate::support::select;

use artifacts::{check_archive, check_directory, check_dockerfile, check_image};
use inside::{check_inside, check_sandbox};

/// project scopeの状態値。翻訳しない安定したenum。
///
/// `unknown`は使用しない。観測していない項目は、観測できなかった理由を持つ値で示す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    Ready,
    Missing,
    Mismatch,
    Changed,
    Running,
    Stopped,
    NotCreated,
    Clean,
    Dirty,
    Attached,
    Detached,
    NotExposed,
    Exposed,
    NotApplicable,
    NotObservedStopped,
}
impl Value {
    pub fn as_str(self) -> &'static str {
        match self {
            Value::Ready => "ready",
            Value::Missing => "missing",
            Value::Mismatch => "mismatch",
            Value::Changed => "changed",
            Value::Running => "running",
            Value::Stopped => "stopped",
            Value::NotCreated => "not-created",
            Value::Clean => "clean",
            Value::Dirty => "dirty",
            Value::Attached => "attached",
            Value::Detached => "detached",
            Value::NotExposed => "not-exposed",
            Value::Exposed => "exposed",
            Value::NotApplicable => "not-applicable",
            Value::NotObservedStopped => "not-observed-stopped",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Value::Ready => "legend-ready",
            Value::Missing => "legend-missing",
            Value::Mismatch => "legend-mismatch",
            Value::Changed => "legend-changed",
            Value::Running => "legend-sandbox-running",
            Value::Stopped => "legend-sandbox-stopped",
            Value::NotCreated => "legend-not-created",
            Value::Clean => "legend-clean",
            Value::Dirty => "legend-dirty",
            Value::Attached => "legend-attached",
            Value::Detached => "legend-detached",
            Value::NotExposed => "legend-not-exposed",
            Value::Exposed => "legend-exposed",
            Value::NotApplicable => "legend-not-applicable",
            Value::NotObservedStopped => "legend-not-observed-stopped",
        }
    }
}
/// 1件の項目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 項目名のFTL message ID。
    pub item: &'static str,
    pub value: Value,
}
/// worktree 1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    /// metadataとの対応。翻訳しない。
    pub kind: &'static str,
    pub mode: Value,
    pub state: Value,
}
/// 診断結果。
#[derive(Debug, Clone)]
pub struct ProjectStatus {
    pub project: String,
    pub items: Vec<Item>,
    pub worktrees: Vec<WorktreeRow>,
    pub diagnostics: Vec<Diagnostic>,
}
impl ProjectStatus {
    pub fn is_healthy(&self) -> bool {
        self.diagnostics.is_empty()
    }

    fn push(&mut self, item: &'static str, value: Value) {
        self.items.push(Item { item, value });
    }

    /// global環境を読めなかったため観測できなかったことを、別commandの案内とともに残す。
    fn global_scope_failure(&mut self, error: &Error) {
        self.diagnostics.extend(error.diagnostics().iter().cloned());
        self.diagnostics.push(
            Diagnostic::new(
                ErrorId::GlobalScopeUnobservable,
                msg!("error-global-scope-unobservable"),
            )
            .remediation(msg!(
                "remediation-run-global-status",
                command = "sbxm status --global"
            )),
        );
    }
}
/// 1案件を診断する。何も変更しない。
pub fn diagnose(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<ProjectStatus> {
    let canonical = project.canonical();
    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    let Some(metadata) = crate::metadata::load(&paths)? else {
        return Err(select::not_managed(project));
    };
    let name = SandboxName::derive(&canonical);

    let mut status = ProjectStatus {
        project: metadata.display_id(),
        items: Vec::new(),
        worktrees: Vec::new(),
        diagnostics: Vec::new(),
    };

    // 1. metadataと目標構成
    status.push("status-item-metadata", Value::Ready);

    // 2. project rootとhost clone
    check_directory(&paths, &mut status);

    // 3. Dockerfileの世代
    check_dockerfile(&paths, &metadata, &mut status);

    // 4-5. image、archive、Sandbox
    check_image(host, &name, &metadata, &mut status);
    check_archive(&paths, &metadata, &mut status);
    let state = check_sandbox(host, &metadata, workspace_root, &mut status);

    // 6-10. Sandbox内部の検査
    check_inside(host, &name, &metadata, state, &mut status);

    Ok(status)
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
