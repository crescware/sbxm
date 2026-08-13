use std::collections::BTreeSet;
use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::SandboxEntry;
use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::paths::ProjectPaths;
use crate::registry::{self};

use crate::support::daemon;

use super::{
    ManagedProject, Observed, ProjectState, Snapshot, duplicated, observe, observe_workspace,
};

/// 現在のinventoryを1回の一覧取得から組み立てる。
///
/// registryが読めない、Sandbox名が重複、対応が矛盾する場合は、部分的に正しそうな
/// 結果を返さずerrorとする。個々の案件の観測結果は、entryを黙って落とさずそのまま返す。
///
/// workspace directoryの実在は、runtime stateとは別の事実として1案件ずつ観測する。
/// 観測できない案件があってもerrorにしない。1案件の破損で一覧全体を失わせない。
pub fn take(
    location: &ConfigLocation,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<Snapshot> {
    let registry = registry::load(location)?;
    let entries = daemon::list(host)?;
    require_unique_names(&entries)?;

    let mut projects = Vec::with_capacity(registry.entries().len());
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    for entry in registry.entries() {
        let name = entry.sandbox_name();
        let paths = ProjectPaths::at(entry.project_root(), entry.canonical_id());
        let observed = observe(&paths, entry, &entries, workspace_root)?;
        if matches!(
            observed,
            Observed::Registered(ProjectState::Running | ProjectState::Stopped)
        ) {
            claimed.insert(name.as_str().to_string());
        }
        projects.push(ManagedProject {
            display_id: entry.repository().display_id(),
            project_root: entry.project_root().to_path_buf(),
            sandbox: name.as_str().to_string(),
            workspace: observe_workspace(&name, workspace_root, &observed),
            observed,
        });
    }

    let mut unmanaged: Vec<SandboxEntry> = entries
        .into_iter()
        .filter(|entry| !claimed.contains(&entry.name))
        .collect();
    unmanaged.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));

    Ok(Snapshot {
        projects,
        unmanaged,
    })
}

/// 同名のSandboxが複数ある一覧からは、対応を決められない。
fn require_unique_names(entries: &[SandboxEntry]) -> Result<()> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut duplicates: BTreeSet<&str> = BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.name.as_str()) {
            duplicates.insert(entry.name.as_str());
        }
    }
    if duplicates.is_empty() {
        return Ok(());
    }
    Err(duplicated(&duplicates.into_iter().collect::<Vec<&str>>()))
}
