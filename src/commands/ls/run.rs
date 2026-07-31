//! `sbxm ls`。
//!
//! 一覧はglobal registryから作る。base pathの走査もcwdからの推測も行わない。
//! entryが指すpathが消えていても黙って落とさず、観測した状態をそのまま並べる。
//! 取り込みも削除も行わない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::error::Result;

use crate::support::inventory::{self, Observed};

/// 一覧の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub project: String,
    /// registryが指すhost project root。
    pub root: String,
    pub sandbox: String,
    pub observed: Observed,
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
    /// 全案件が登録済みで、成果物がregistryと一致しているか。
    pub settled: bool,
}

/// 管理案件と管理外Sandboxを一覧する。
pub fn run(
    location: &ConfigLocation,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<Listing> {
    let inventory = inventory::take(location, host, workspace_root)?;

    Ok(Listing {
        settled: inventory.is_settled(),
        projects: inventory
            .projects
            .iter()
            .map(|project| ProjectRow {
                project: project.display_id.clone(),
                root: crate::paths::display(&project.project_root),
                sandbox: project.sandbox.clone(),
                observed: project.observed.clone(),
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
