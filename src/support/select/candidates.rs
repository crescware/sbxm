use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::paths::ProjectPaths;
use crate::registry;

use super::Candidate;

/// registryが持つ案件をpromptへ並べる順に返す。canonical ID昇順で返す。
pub fn candidates(location: &ConfigLocation) -> Result<Vec<Candidate>> {
    let registry = registry::load(location)?;
    Ok(registry
        .entries()
        .iter()
        .map(|entry| Candidate {
            paths: ProjectPaths::at(entry.project_root(), entry.canonical_id()),
            repository: entry.repository().clone(),
        })
        .collect())
}
