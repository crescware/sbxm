use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg};
use crate::msg;
use crate::paths::{self};

/// fieldの値が受け付けられないことを報告する。
///
/// 受け付けられない理由はsbxm自身の観測であり、外部の原文ではない。文字列ではなく
/// messageで受け取り、翻訳の対象に留める。
pub(super) fn invalid_value(path: &Path, field: &'static str, detail: Msg) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ConfigInvalidValue,
            msg!("error-config-invalid-value"),
        )
        .fact(Fact::path(&paths::display(path)))
        .fact(Fact::field(field))
        .fact(Fact::reason(detail))
        .remediation(msg!("remediation-fix-config", path = paths::display(path))),
    )
}
