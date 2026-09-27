use crate::design::Fact;
use crate::diagnostics::{Error, ErrorId};
use crate::paths::scope::PathScope;
use std::cell::Cell;
use std::fs::{File, OpenOptions, TryLockError};
use std::time::Duration;

use crate::testing::outcome::{Checked, Refused, Required};

use super::acquire_lock::acquire_lock;
use super::*;
use std::fs;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

use crate::paths::{LOCK_TIMEOUT, PRIVATE_FILE_MODE};
use crate::testing::fs::temp_dir;
use crate::testing::scripted_clock::ScriptedClock;
use crate::time::{Clock, Moment};

/// 診断が挙げた事実のうち、OSが書いた原文。
fn cause_of(error: &Error) -> Checked<String> {
    error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-cause-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the cause is quoted from the operating system")
}

#[test]
fn an_exclusive_lock_keeps_a_second_holder_out_until_it_is_released() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("init.lock");

    let held = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .required_because("acquire")?;

    // 期限を0にすると、1度だけ待たずに試して答える。待つ判断は台本の時計で確かめる。
    let error = acquire_exclusive_lock(
        &path,
        Duration::ZERO,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .refused_because("a second holder does not get the lock")?;
    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

    drop(held);

    acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .required_because("the lock can be taken again once the first holder releases it")?;
    Ok(())
}

#[test]
fn a_lock_that_stays_held_is_tried_again_until_the_deadline() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("init.lock");
    let clock = ScriptedClock::default();
    let timeout = Duration::from_millis(100);

    let error = acquire_lock(
        &path,
        timeout,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
        &|_| Err(TryLockError::WouldBlock),
        &clock,
    )
    .refused_because("a lock that is never released is not waited for forever")?;

    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));
    // 25msおきに試し直し、期限ちょうどの試行で諦める。
    assert_eq!(clock.now(), Moment::since_origin(timeout));
    assert_eq!(*clock.slept.borrow(), vec![Duration::from_millis(25); 4]);
    Ok(())
}

#[test]
fn a_lock_that_must_not_wait_is_tried_once() -> Checked {
    // lockが取られているかを見るtestは、期限0で待たずに1度だけ試すことに頼る。
    let dir = temp_dir()?;
    let path = dir.path().join("init.lock");
    let clock = ScriptedClock::default();
    let tries = Cell::new(0);

    let error = acquire_lock(
        &path,
        Duration::ZERO,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
        &|_| {
            tries.set(tries.get() + 1);
            Err(TryLockError::WouldBlock)
        },
        &clock,
    )
    .refused_because("a held lock is not taken")?;

    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));
    assert_eq!(tries.get(), 1);
    assert!(clock.slept.borrow().is_empty(), "it does not wait");
    Ok(())
}

#[test]
fn a_lock_released_while_waiting_is_taken() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("init.lock");
    let clock = ScriptedClock::default();
    let tries = Cell::new(0);

    acquire_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
        &|_| {
            tries.set(tries.get() + 1);
            if tries.get() < 3 {
                Err(TryLockError::WouldBlock)
            } else {
                Ok(())
            }
        },
        &clock,
    )
    .required_because("the holder releases the lock on the third try")?;

    assert_eq!(tries.get(), 3);
    assert_eq!(
        clock.slept.borrow().len(),
        2,
        "it waited once between tries"
    );
    Ok(())
}

#[test]
fn a_lock_that_cannot_be_tried_reports_what_the_operating_system_said() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("init.lock");

    let error = acquire_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
        &|_| {
            Err(TryLockError::Error(std::io::Error::other(
                "the file system does not lock",
            )))
        },
        &ScriptedClock::default(),
    )
    .refused_because("a lock that cannot be tried is not waited for")?;

    assert_eq!(error.first_id(), Some(ErrorId::LockUnavailable));
    assert_eq!(cause_of(&error)?, "the file system does not lock");
    Ok(())
}

#[test]
fn a_lock_file_that_is_not_private_is_never_taken() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    fs::write(&path, b"").required_because("seed the lock file")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).required_because("widen")?;

    let error = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("a lock other accounts can take is not a lock")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    let mode = fs::metadata(&path).required()?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o666, "sbxm must not repair permissions on its own");
    Ok(())
}

#[test]
fn a_symlinked_lock_path_is_reported_for_the_scope_it_protects() -> Checked {
    let dir = temp_dir()?;
    let real = dir.path().join("real.lock");
    fs::write(&real, b"").required()?;
    let link = dir.path().join("project.lock");
    std::os::unix::fs::symlink(&real, &link).required()?;

    for (scope, expected) in [
        (PathScope::ProjectPath, ErrorId::ProjectPathSymlink),
        (PathScope::ConfigFile, ErrorId::ConfigSymlink),
    ] {
        let error = acquire_exclusive_lock(&link, LOCK_TIMEOUT, PRIVATE_FILE_MODE, scope)
            .refused_because("symlinked lock paths are refused")?;
        assert_eq!(error.first_id(), Some(expected));
    }
    Ok(())
}

#[test]
fn a_lock_file_survives_the_workflow_that_created_it() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("init.lock");
    {
        let _lock = acquire_exclusive_lock(
            &path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ConfigFile,
        )
        .required_because("acquire")?;
    }
    assert!(
        path.exists(),
        "the lock file is not deleted when the workflow ends"
    );
    Ok(())
}

#[test]
fn a_lock_path_that_cannot_be_opened_reports_what_the_operating_system_said() -> Checked {
    let dir = temp_dir()?;
    // directoryはlock fileとして開けない。取れなかった理由をOSの原文のまま示す。
    let path = dir.path().join("project.lock");
    fs::create_dir(&path).required_because("create a directory in the lock's place")?;

    let error = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("a lock file that cannot be opened is not waited for")?;
    assert_eq!(error.first_id(), Some(ErrorId::LockUnavailable));
    assert!(
        !cause_of(&error)?.is_empty(),
        "the operating system said why"
    );
    assert!(path.is_dir(), "the path that was in the way is untouched");
    Ok(())
}

/// 待っているあいだに、lock fileのpathを別のfileへ置き換える。
fn replace_the_lock_file(
    directory: &std::path::Path,
    path: &std::path::Path,
) -> std::io::Result<()> {
    let replacement = directory.join("replacement.lock");
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(PRIVATE_FILE_MODE)
        .open(&replacement)?;
    fs::rename(&replacement, path)
}

#[test]
fn a_lock_replaced_while_waiting_is_reopened_before_returning() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    let replaced = Cell::new(false);

    let file = acquire_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
        &|_: &File| {
            // 古いfileのlockが取れたときには、pathはもう別のfileを指している。
            if !replaced.replace(true) {
                replace_the_lock_file(dir.path(), &path).map_err(TryLockError::Error)?;
            }
            Ok(())
        },
        &ScriptedClock::default(),
    )
    .required_because("the file now at the path is locked")?;

    assert_eq!(
        file.metadata().required()?.ino(),
        fs::metadata(&path).required()?.ino(),
        "the lock protects the file that is at the path when it returns"
    );
    Ok(())
}

#[test]
fn a_lock_replaced_at_the_deadline_is_not_taken() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    let clock = ScriptedClock::default();
    let tries = Cell::new(0);

    let error = acquire_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
        &|_: &File| {
            tries.set(tries.get() + 1);
            replace_the_lock_file(dir.path(), &path).map_err(TryLockError::Error)?;
            clock.advance(LOCK_TIMEOUT);
            Ok(())
        },
        &clock,
    )
    .refused_because("a lock on a file that was replaced protects nothing")?;

    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));
    assert_eq!(
        tries.get(),
        1,
        "the deadline has passed, so it does not reopen"
    );
    Ok(())
}
