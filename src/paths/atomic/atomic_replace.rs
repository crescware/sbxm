use std::path::Path;

use crate::diagnostics::Result;

use super::{atomic_write_with_precondition, replaceable_identity, unchanged_identity};

/// 既存fileをatomicに置き換える。
///
/// 置き換えて良い相手であることを、file type、permission、identityで確認してから書く。
/// rename直前に同じ検査をやり直し、書いている間に別の実体へ差し替えられていた場合は
/// 何も上書きしない。
pub fn atomic_replace(target: &Path, contents: &str, mode: u32) -> Result<()> {
    let expected = replaceable_identity(target, mode)?;
    atomic_write_with_precondition(target, contents, mode, move |target| {
        unchanged_identity(target, mode, expected)
    })
}
