use crate::msg;
use crate::paths::{self, ProjectPaths};

use super::Target;

/// 保持対象。
pub(super) fn keeps(paths: &ProjectPaths) -> Vec<Target> {
    vec![
        Target::Path(paths::display(&paths.host_clone())),
        Target::Path(paths::display(&paths.dockerfile())),
        Target::Described(msg!("destroy-target-host-images")),
        Target::Described(msg!("destroy-target-secrets")),
    ]
}
