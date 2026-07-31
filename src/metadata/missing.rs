use std::path::Path;

use crate::diagnostics::{Error, ErrorId};
use crate::msg;
use crate::paths::{self};

/// 必須fieldが無いことを報告する。
pub(super) fn missing(path: &Path, field: &'static str) -> Error {
    Error::new(
        ErrorId::MetadataMissingField,
        msg!(
            "error-metadata-missing-field",
            path = paths::display(path),
            field = field
        ),
    )
}
