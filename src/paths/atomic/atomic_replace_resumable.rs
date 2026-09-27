use std::path::Path;

use crate::diagnostics::Result;

use super::{replaceable_identity, resumable_write_with_precondition, unchanged_identity};

/// 中断した別実行の一時fileに妨げられず、既存fileをatomicに置き換える。
///
/// `atomic_replace`と同じく、rename直前にidentityの検査をやり直し、書いている間に別の
/// 実体へ差し替えられていた場合は何も上書きしない。
pub fn atomic_replace_resumable(target: &Path, contents: &str, mode: u32) -> Result<()> {
    let expected = replaceable_identity(target, mode)?;
    resumable_write_with_precondition(target, contents, mode, move |target| {
        unchanged_identity(target, mode, expected)
    })
}
