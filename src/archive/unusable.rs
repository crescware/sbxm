use std::path::Path;

use crate::diagnostics::{Error, ErrorId};
use crate::msg;
use crate::paths;

pub(super) fn unusable(path: &Path, detail: &str) -> Error {
    Error::new(
        ErrorId::ArchiveUnusable,
        msg!(
            "error-archive-unusable",
            path = paths::display(path),
            detail = detail
        ),
    )
}
