use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::diagnostics::Result;

use super::atomic_write_failed;

/// 中断した別実行の一時fileに妨げられないatomic writeの共通部分。
///
/// 一時fileの名前は実行ごとに変わり、残骸があっても作成は失敗しない。
///
/// 1. 同一directoryに一時fileを作る
/// 2. 必要permissionを設定する
/// 3. 全内容を書いて`sync_all`する
/// 4. 呼び出し側のprecondition検査を通す
/// 5. renameする
/// 6. 親directoryを`sync_all`する
pub(super) fn resumable_write_with_precondition(
    target: &Path,
    contents: &str,
    mode: u32,
    precondition: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
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
    precondition(target)?;
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
