use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::diagnostics::Result;

use super::{atomic_write_failed, replaceable_identity, unchanged_identity};

/// 中断した別実行の一時fileに妨げられず、既存fileをatomicに置き換える。
pub fn atomic_replace_resumable(target: &Path, contents: &str, mode: u32) -> Result<()> {
    let expected = replaceable_identity(target, mode)?;
    let Some(parent) = target.parent() else {
        return Err(atomic_write_failed(target, "missing parent"));
    };
    let mut temporary = write_result(
        target,
        tempfile::Builder::new()
            .prefix(".project.yaml.")
            .tempfile_in(parent),
    )?;
    write_result(
        target,
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(mode)),
    )?;
    write_result(target, temporary.write_all(contents.as_bytes()))?;
    write_result(target, temporary.as_file().sync_all())?;
    unchanged_identity(target, mode, expected)?;
    if let Err(error) = temporary.persist(target) {
        return Err(atomic_write_failed(target, &error.error.to_string()));
    }
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

fn write_result<T>(target: &Path, result: std::io::Result<T>) -> Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(atomic_write_failed(target, &error.to_string())),
    }
}
