//! OS file lockによる保護区間。

use std::fs::{File, OpenOptions, TryLockError};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;

use super::inspect::{FileIdentity, display, is_symlink, require_private_file};
use super::scope::PathScope;

const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// 保持している間だけ保護区間を占有するOS file lock。
///
/// lock fileはworkflow終了後も削除しない。fileの存在自体は処理中を意味せず、
/// lock取得の成否だけが排他の根拠となる。
#[derive(Debug)]
pub struct ExclusiveLock {
    file: File,
}

impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// exclusiveなOS file lockをtimeout付きで取得する。
///
/// lock取得後、開いたfileと現在のlock pathのidentityが一致することを確認し、
/// 一致しない場合は削除済みの古いlock fileとみなして取り直す。
///
/// permission、file type、所有関係が不正なlock fileは、安全に置換できないため
/// 取得せずに終了する。所有関係は、fileの所有者が現在の利用者であることで判定する。
pub fn acquire_exclusive_lock(
    path: &Path,
    timeout: Duration,
    mode: u32,
    scope: PathScope,
) -> Result<ExclusiveLock> {
    let deadline = Instant::now() + timeout;
    loop {
        if is_symlink(path) {
            return Err(scope.symlink_error(path));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(mode)
            .open(path)
            .map_err(|error| {
                Error::new(
                    ErrorId::LockUnavailable,
                    msg!(
                        "error-lock-unavailable",
                        path = display(path),
                        detail = error
                    ),
                )
            })?;
        require_private_file(&file, path, mode, scope)?;

        let acquired = loop {
            match file.try_lock() {
                Ok(()) => break true,
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        break false;
                    }
                    std::thread::sleep(LOCK_POLL_INTERVAL);
                }
                Err(TryLockError::Error(error)) => {
                    return fail(
                        ErrorId::LockUnavailable,
                        msg!(
                            "error-lock-unavailable",
                            path = display(path),
                            detail = error
                        ),
                    );
                }
            }
        };

        if !acquired {
            return Err(timed_out(path, timeout));
        }

        let open_identity = FileIdentity::of_open_file(&file).map_err(|error| {
            Error::new(
                ErrorId::LockUnavailable,
                msg!(
                    "error-lock-unavailable",
                    path = display(path),
                    detail = error
                ),
            )
        })?;
        match FileIdentity::of_path_without_following(path) {
            Ok(path_identity) if path_identity == open_identity => {
                return Ok(ExclusiveLock { file });
            }
            // 待機中にlock fileが削除・再作成された。古いinodeのlockは保護にならない。
            _ => {
                let _ = file.unlock();
                drop(file);
                if Instant::now() >= deadline {
                    return Err(timed_out(path, timeout));
                }
            }
        }
    }
}

fn timed_out(path: &Path, timeout: Duration) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::LockTimeout,
            msg!(
                "error-lock-timeout",
                path = display(path),
                seconds = timeout.as_secs()
            ),
        )
        .remediation(msg!("remediation-wait-for-lock")),
    )
}

#[cfg(test)]
#[path = "lock_test.rs"]
mod lock_test;
