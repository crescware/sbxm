use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, ProjectPaths};
use crate::support::repository;

use super::Target;

/// 保持対象。
///
/// hostにあるrepositoryを登録した案件では、登録したそのrepositoryを残す。
pub(super) fn keeps(paths: &ProjectPaths, metadata: &ProjectMetadata) -> Vec<Target> {
    vec![
        Target::Path(paths::display(&repository::host_repository(
            paths, metadata,
        ))),
        Target::Path(paths::display(&paths.dockerfile())),
        Target::Described(msg!("destroy-target-host-images")),
        Target::Described(msg!("destroy-target-secrets")),
    ]
}
