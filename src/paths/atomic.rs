//! atomic write。
//!
//! 既存fileを暗黙に削除または上書きせず、config・metadataは検証を通してから
//! 置き換える。

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;

use super::inspect::{FileIdentity, display, is_symlink, permission_too_open, unexpected_type};
use super::scope::PathScope;

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

/// 既存fileをatomicに置き換える。
///
/// 置き換えて良い相手であることを、file type、permission、identityで確認してから書く。
/// rename直前に同じ検査をやり直し、書いている間に別の実体へ差し替えられていた場合は
/// 何も上書きしない。
pub fn atomic_replace(target: &Path, contents: &str, mode: u32) -> Result<()> {
    let expected = replaceable_identity(target, mode)?;
    atomic_write_with_precondition(target, contents, mode, move |target| {
        let observed = replaceable_identity(target, mode)?;
        if observed != expected {
            return fail(
                ErrorId::TargetChangedConcurrently,
                msg!("error-target-changed-concurrently", path = display(target)),
            );
        }
        Ok(())
    })
}

/// 検証済みの一時fileを、同じdirectory内の正式pathへatomicに移す。
///
/// 内容をこのprocessが組み立てられない成果物、たとえば外部commandが書いたarchiveを、
/// 検証を終えてから置き換えるために使う。
pub fn atomic_rename_into_place(temp: &Path, target: &Path) -> Result<()> {
    if is_symlink(temp) {
        return Err(PathScope::ProjectPath.symlink_error(temp));
    }
    if is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(temp)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(temp, &error.to_string()))?;
    if !metadata.is_file() {
        return Err(unexpected_type(temp, "regular file", &metadata));
    }

    fs::rename(temp, target).map_err(|error| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = error
            ),
        )
    })?;
    if let Some(parent) = target.parent()
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// atomic writeの共通部分。
///
/// 1. 同一directoryに`create_new`で一時fileを作る
/// 2. 必要permissionを設定する
/// 3. 全内容を書いて`sync_all`する
/// 4. 呼び出し側のprecondition検査を通す
/// 5. renameする
/// 6. 親directoryを`sync_all`する
fn atomic_write_with_precondition(
    target: &Path,
    contents: &str,
    mode: u32,
    precondition: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let temp = temp_path_for(target)?;
    let write_failed = |detail: String| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = detail
            ),
        )
    };

    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::TempFileLeftBehind,
                    msg!("error-temp-file-left-behind", path = display(&temp)),
                )
                .remediation(msg!("remediation-remove-temp-file", path = display(&temp))),
            ));
        }
        Err(error) => return Err(write_failed(error.to_string())),
    };

    let result = (|| -> Result<()> {
        // umaskの影響を受けずに、要求したpermissionを確定させる。
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| write_failed(error.to_string()))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| write_failed(error.to_string()))?;
        file.sync_all()
            .map_err(|error| write_failed(error.to_string()))?;
        precondition(target)?;
        fs::rename(&temp, target).map_err(|error| write_failed(error.to_string()))?;
        if let Some(parent) = target.parent()
            && let Ok(directory) = File::open(parent)
        {
            // rename自体を永続化する。失敗しても書き込み内容は失われないため、致命的には扱わない。
            let _ = directory.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        // rename前の失敗では、この実行が作った一時fileだけを片付ける。
        let _ = fs::remove_file(&temp);
    }
    result
}

/// 決定的な一時file名。中断した実行の残骸を次回起動時に検出できるようにする。
fn temp_path_for(target: &Path) -> Result<PathBuf> {
    let parent = target.parent().ok_or_else(|| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = "the target has no parent directory"
            ),
        )
    })?;
    let name = target.file_name().ok_or_else(|| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = "the target has no file name"
            ),
        )
    })?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(name);
    temp_name.push(".tmp");
    Ok(parent.join(temp_name))
}

/// 置き換え対象として妥当なfileのidentity。
fn replaceable_identity(target: &Path, mode: u32) -> Result<FileIdentity> {
    if is_symlink(target) {
        return Err(PathScope::ProjectPath.symlink_error(target));
    }
    let metadata = fs::symlink_metadata(target).map_err(|error| {
        Error::new(
            ErrorId::AtomicWriteFailed,
            msg!(
                "error-atomic-write-failed",
                path = display(target),
                detail = error
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(unexpected_type(target, "regular file", &metadata));
    }
    let observed = metadata.permissions().mode();
    if permission_too_open(observed) {
        return Err(PathScope::ProjectPath.permission_error(target, observed, mode));
    }
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(test)]
#[path = "atomic_test.rs"]
mod atomic_test;
