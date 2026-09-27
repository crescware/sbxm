//! 契約test: file lockについての仮定。
//!
//! lockの取り直しの判断は、次を前提にする。lockは開いたfileごとに掛かり、同じprocessの中でも
//! 別に開いたfileどうしで排他する。外すか閉じれば解ける。pathではなく開いたfileに掛かるため、
//! pathを別のfileへ置き換えれば、開き直したfileにはlockが無い。
//!
//! どれも1つのthreadで、待たずに試した結果だけを見る。閉じたことで解けるのを見る仮定だけは、
//! 解けるまで待つ。同じprocessの別のthreadのtestがforkした子は、自分のexecまで開いたfileを
//! 共有し、そのlockも持つためである。

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

use crate::boundary::os::SystemClock;
use crate::testing::fs::temp_dir;
use crate::testing::outcome::{Checked, Required, Unmet};
use crate::testing::wait_until::wait_until;

use super::*;

fn open(path: &Path) -> Checked<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .required_because("the lock file opens")
}

fn would_block(result: &Result<(), TryLockError>) -> bool {
    matches!(result, Err(TryLockError::WouldBlock))
}

#[test]
fn an_exclusive_lock_excludes_every_other_opening_of_the_file() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    let held = open(&path)?;
    let other = open(&path)?;

    SystemFileLock::try_lock(&held).required_because("the first lock is free")?;

    assert!(would_block(&SystemFileLock::try_lock(&other)));
    assert!(would_block(&SystemFileLock::try_lock_shared(&other)));
    Ok(())
}

#[test]
fn shared_locks_coexist_and_keep_an_exclusive_one_out() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("session.lock");
    let first = open(&path)?;
    let second = open(&path)?;
    let exclusive = open(&path)?;

    SystemFileLock::try_lock_shared(&first).required_because("a shared lock is free")?;
    SystemFileLock::try_lock_shared(&second).required_because("shared locks coexist")?;

    assert!(would_block(&SystemFileLock::try_lock(&exclusive)));
    Ok(())
}

#[test]
fn closing_the_file_releases_its_lock() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    let held = open(&path)?;
    SystemFileLock::try_lock(&held).required_because("the first lock is free")?;
    drop(held);

    let reopened = open(&path)?;
    wait_until(
        &SystemClock,
        "closing to release the lock",
        || match SystemFileLock::try_lock(&reopened) {
            Ok(()) => Ok(Some(())),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(Unmet::new(format!(
                "the lock could not be tried: {error:?}"
            ))),
        },
    )
}

#[test]
fn unlocking_releases_the_lock_while_the_file_stays_open() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    let held = open(&path)?;
    SystemFileLock::try_lock(&held).required_because("the first lock is free")?;
    SystemFileLock::unlock(&held).required_because("the lock is released")?;

    SystemFileLock::try_lock(&open(&path)?).required_because("the released lock is free")?;
    drop(held);
    Ok(())
}

#[test]
fn a_lock_belongs_to_the_opened_file_and_not_to_its_path() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    let held = open(&path)?;
    SystemFileLock::try_lock(&held).required_because("the first lock is free")?;

    let replacement = dir.path().join("replacement.lock");
    open(&replacement)?;
    std::fs::rename(&replacement, &path).required_because("replace the lock path")?;

    SystemFileLock::try_lock(&open(&path)?)
        .required_because("the file now at the path carries no lock")?;
    Ok(())
}
