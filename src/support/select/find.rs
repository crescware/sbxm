use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::paths::ProjectPaths;
use crate::project::ProjectId;
use crate::registry;

use super::{Candidate, not_managed};

/// 完全指定された案件のmetadataを、導出したpathから読む。
///
/// 探索を行わないため、対象と無関係な案件の状態に左右されない。
pub fn find(location: &ConfigLocation, project: &ProjectId) -> Result<Candidate> {
    let registry = registry::load(location)?;
    let canonical = project.canonical();
    match registry.find(&canonical) {
        Some(entry) => Ok(Candidate {
            paths: ProjectPaths::at(entry.project_root(), &canonical),
            repository: entry.repository().clone(),
        }),
        None => Err(not_managed(project)),
    }
}
