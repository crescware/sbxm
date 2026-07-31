use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::metadata::{self};
use crate::paths::{self, PathScope, ProjectPaths};
use crate::registry::{self};
use crate::repository::RepositoryIdentity;

use super::{Presence, observe};

/// この実行の前から、metadataまで揃った登録済み案件だったか。
///
/// registry entryのabsolute pathを、読む前に観測する。保存されたrootであっても、
/// そこにdirectoryがあり、現在の利用者が所有していることを確かめてからmetadataを読む。
/// rootがまだ無い中断点は登録途中であり、観測できないrootは不在と同一視しない。
pub(super) fn was_already_registered(
    location: &ConfigLocation,
    repository: &RepositoryIdentity,
) -> Result<bool> {
    let registry = registry::load(location)?;
    let Some(entry) = registry.find(repository.canonical_id()) else {
        return Ok(false);
    };
    let paths = ProjectPaths::at(entry.project_root(), entry.canonical_id());
    match observe(paths.root())? {
        Presence::Absent => Ok(false),
        Presence::Present => {
            paths::require_owned_directory(paths.root(), PathScope::ProjectPath)?;
            Ok(metadata::load(&paths)?.is_some())
        }
    }
}
