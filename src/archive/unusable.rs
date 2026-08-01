use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;
use crate::paths;

/// archiveを受け付けられないことを報告する。
///
/// archiveの中身をどう読めなかったかはsbxm自身の観測であり、外部の原文ではない。
pub(super) fn unusable(path: &Path, reason: Msg) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::ArchiveUnusable, msg!("error-archive-unusable"))
            .fact(Fact::path(&paths::display(path)))
            .fact(Fact::reason(reason)),
    )
}
