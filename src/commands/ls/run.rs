use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::ConfigLocation;
use crate::diagnostics::Result;

use crate::support::inventory::{self};

use super::{Listing, ProjectRow, UnmanagedRow};

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
                workspace: project.workspace,
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
