use std::fs::File;
use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// rename済みのdirectory entryを永続化する。失敗時も配置済みfileは戻さない。
pub fn sync_parent_directory(target: &Path) -> Result<()> {
    let sync = || {
        let parent = target
            .parent()
            .ok_or_else(|| std::io::Error::other("missing parent directory"))?;
        File::open(parent)?.sync_all()
    };
    checked(target, sync())
}

fn checked(target: &Path, result: std::io::Result<()>) -> Result<()> {
    result.map_err(|error| {
        Error::single(
            Diagnostic::new(
                ErrorId::AtomicWriteDurabilityUnconfirmed,
                msg!("error-atomic-write-durability-unconfirmed"),
            )
            .fact(Fact::path(&super::display(target)))
            .fact(Fact::cause(&error.to_string()))
            .remediation(msg!("remediation-atomic-write-durability-unconfirmed")),
        )
    })
}

#[cfg(test)]
#[path = "sync_parent_directory_test.rs"]
mod sync_parent_directory_test;
