use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::Error;

use super::reported;

/// `OSやparserが書いた原文`をそのまま原因として示す。
///
/// sbxm側の言い換えを被せると、翻訳された文へ英語を連結することになる。どのentryの
/// 話かは`Entry:`の行が示すため、原因は観測されたままでよい。
pub(super) fn unreadable(path: &Path, entry: Option<&str>, detail: &str) -> Error {
    let mut diagnostic = reported(path);
    if let Some(entry) = entry {
        diagnostic = diagnostic.fact(Fact::entry(entry));
    }
    Error::single(diagnostic.fact(Fact::cause(detail)))
}
