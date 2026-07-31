use std::path::Path;

use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::paths::{self};

/// fieldの値が受け付けられないことを報告する。
pub(super) fn invalid_value(path: &Path, field: &'static str, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ConfigInvalidValue,
            msg!(
                "error-config-invalid-value",
                path = paths::display(path),
                field = field,
                detail = detail
            ),
        )
        .remediation(msg!("remediation-fix-config", path = paths::display(path))),
    )
}
