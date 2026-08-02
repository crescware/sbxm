use std::os::unix::fs::PermissionsExt;

use crate::diagnostics::Result;
use crate::paths::PRIVATE_DIR_MODE;

use super::{BUILD_CONTEXT_PREFIX, empty_private_context};

/// `docker build`へ渡す、空で私有な一時directory。
pub(super) fn ephemeral_context() -> Result<tempfile::TempDir> {
    empty_private_context(
        tempfile::Builder::new()
            .prefix(BUILD_CONTEXT_PREFIX)
            .permissions(std::fs::Permissions::from_mode(PRIVATE_DIR_MODE))
            .tempdir(),
    )
}
