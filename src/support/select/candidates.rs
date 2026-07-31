use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::paths::ProjectPaths;
use crate::registry;

use super::{Candidate, no_managed_projects};

/// promptへ並べる候補。canonical ID昇順で、0件は選択を開始できないerrorとする。
pub(super) fn candidates(location: &ConfigLocation) -> Result<Vec<Candidate>> {
    let registry = registry::load(location)?;
    if registry.entries().is_empty() {
        // 候補0件は、選択を取り消した状態ではなく対象選択を開始できないerrorである。
        return Err(no_managed_projects());
    }
    Ok(registry
        .entries()
        .iter()
        .map(|entry| Candidate {
            paths: ProjectPaths::at(entry.project_root(), entry.canonical_id()),
            repository: entry.repository().clone(),
        })
        .collect())
}
