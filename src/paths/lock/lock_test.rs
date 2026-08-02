use crate::design::Fact;
use crate::diagnostics::{Error, ErrorId};
use crate::paths::scope::PathScope;
use std::time::Duration;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::thread;

use crate::paths::{LOCK_TIMEOUT, PRIVATE_FILE_MODE};
use crate::testing::fs::temp_dir;

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
fn an_exclusive_lock_serializes_concurrent_holders() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("init.lock");

    let held = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .required_because("acquire")?;

    let contended = {
        let path = path.clone();
        thread::spawn(move || {
            acquire_exclusive_lock(
                &path,
                Duration::from_millis(150),
                PRIVATE_FILE_MODE,
                PathScope::ConfigFile,
            )
            .map(|_| ())
        })
    };
    let error = contended
        .join()
        .required_because("thread joins")?
        .refused_because("a second holder must wait and then time out")?;
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

#[test]
fn a_lock_replaced_while_waiting_is_reopened_before_returning() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    let held = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("hold the original lock")?;

    let waiter_path = path.clone();
    let waiter = thread::spawn(move || {
        acquire_exclusive_lock(
            &waiter_path,
            Duration::from_secs(2),
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
    });
    thread::sleep(Duration::from_millis(100));

    let replacement_path = dir.path().join("replacement.lock");
    let replacement = acquire_exclusive_lock(
        &replacement_path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("hold the replacement inode")?;
    fs::rename(&replacement_path, &path).required_because("replace the lock path")?;
    drop(held);
    thread::sleep(Duration::from_millis(100));
    drop(replacement);

    waiter
        .join()
        .required_because("the waiter joins")?
        .required_because("the waiter acquires the replacement lock")?;
    Ok(())
}
