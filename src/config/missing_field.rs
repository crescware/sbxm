use std::path::Path;

use crate::diagnostics::{Error, ErrorId};
use crate::msg;
use crate::paths::{self};

/// 必須fieldが無いことを報告する。
pub(super) fn missing_field(path: &Path, field: &'static str) -> Error {
    Error::new(
        ErrorId::ConfigMissingField,
        msg!(
            "error-config-missing-field",
            path = paths::display(path),
            field = field
        ),
    )
}
