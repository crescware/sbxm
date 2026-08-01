use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Error, Msg};

use super::reported;

/// archiveを受け付けられないことを報告する。
///
/// archiveの中身をどう読めなかったかはsbxm自身の観測であり、外部の原文ではない。
pub(super) fn unusable(path: &Path, reason: Msg) -> Error {
    Error::single(reported(path).fact(Fact::reason(reason)))
}
