//! `sbxm ls`。
//!
//! 管理案件と、管理外のSandboxを別のtableで一覧する。取り込みも削除も行わない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::Result;

use crate::support::inventory::{self, ProjectState};

/// 一覧の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project: String,
    pub sandbox: String,
    pub state: ProjectState,
}

/// 管理外Sandboxの1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmanagedRow {
    pub sandbox: String,
    /// runtimeが示したままのstate。sbxmのenumへ写像しない。
    pub state: String,
    pub workspace: String,
}

/// `ls`の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub projects: Vec<ProjectRow>,
    pub unmanaged: Vec<UnmanagedRow>,
}

/// 管理案件と管理外Sandboxを一覧する。
pub fn run(
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<Listing> {
    let inventory = inventory::take(config, host, workspace_root)?;

    Ok(Listing {
        projects: inventory
            .projects
            .iter()
            .map(|project| ProjectRow {
                project: project.display_id(),
                sandbox: project.sandbox.as_str().to_string(),
                state: project.state,
            })
            .collect(),
        unmanaged: inventory
            .unmanaged
            .iter()
            .map(|entry| UnmanagedRow {
                sandbox: entry.name.clone(),
                // 管理外Sandboxはsbxmの管理状態を持たないため、原値のまま示す。
                state: entry.raw_state.clone(),
                workspace: entry.workspace.clone().unwrap_or_else(|| "-".to_string()),
            })
            .collect(),
    })
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
