use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;
use crate::paths::inspect::display;

/// atomic writeのI/O失敗を対象pathとOSの原因つきで報告する。
pub(crate) fn atomic_write_failed(target: &Path, detail: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::AtomicWriteFailed,
            msg!("error-atomic-write-failed"),
        )
        .fact(Fact::path(&display(target)))
        .fact(Fact::cause(detail)),
    )
}
