use std::fs::{self};
use std::path::Path;

use crate::diagnostics::{ErrorId, Result, fail};
use crate::msg;

use crate::paths::inspect::display;

use super::atomic_write_with_precondition;

/// 新規fileとしてatomic writeする。既存targetがあれば上書きしない。
pub fn atomic_create(target: &Path, contents: &str, mode: u32) -> Result<()> {
    atomic_write_with_precondition(target, contents, mode, |target| {
        if fs::symlink_metadata(target).is_ok() {
            return fail(
                ErrorId::TargetAppearedConcurrently,
                msg!("error-target-appeared-concurrently", path = display(target)),
            );
        }
        Ok(())
    })
}
