use std::path::Path;

use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use crate::paths::inspect::{FileIdentity, display};

use super::replaceable_identity;

/// rename直前に、置き換える相手が最初に検査したfileのままであることを確かめる。
///
/// 書いているあいだに別のprocessが同じpathを作り直していた場合、そこに在るのは
/// 検査を通した実体ではない。identityが変わっていれば何も上書きしない。
pub(super) fn unchanged_identity(target: &Path, mode: u32, expected: FileIdentity) -> Result<()> {
    let observed = replaceable_identity(target, mode)?;
    if observed != expected {
        return fail(
            ErrorId::TargetChangedConcurrently,
            msg!("error-target-changed-concurrently", path = display(target)),
        );
    }
    Ok(())
}
