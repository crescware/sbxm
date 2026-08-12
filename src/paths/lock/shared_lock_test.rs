use std::time::Duration;

use crate::diagnostics::ErrorId;
use crate::paths::scope::PathScope;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::thread;

use crate::paths::{LOCK_TIMEOUT, PRIVATE_FILE_MODE};
use crate::testing::fs::temp_dir;

#[test]
fn shared_locks_can_be_held_concurrently() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("session.lock");

    let first = acquire_shared_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the first shared holder")?;
    let second = acquire_shared_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("a second shared holder does not wait for the first")?;
    drop(first);
    drop(second);
    Ok(())
}

#[test]
fn an_exclusive_lock_blocks_a_new_shared_lock() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("session.lock");

    let held = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("hold the exclusive lock")?;

    let error = acquire_shared_lock(
        &path,
        Duration::from_millis(150),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("a shared lock must wait behind a held exclusive lock")?;
    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

    drop(held);
    acquire_shared_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the shared lock can be taken once the exclusive lock releases")?;
    Ok(())
}

#[test]
fn a_shared_lock_blocks_a_new_exclusive_lock() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("session.lock");

    let held = acquire_shared_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("hold the shared lock")?;

    let error = acquire_exclusive_lock(
        &path,
        Duration::from_millis(150),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("an exclusive lock must wait behind a held shared lock")?;
    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

    drop(held);
    acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the exclusive lock can be taken once the shared lock releases")?;
    Ok(())
}

#[test]
fn a_shared_lock_file_that_is_not_private_is_never_taken() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("session.lock");
    fs::write(&path, b"").required_because("seed the lock file")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).required_because("widen")?;

    let error = acquire_shared_lock(
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
    Ok(())
}

#[test]
fn a_symlinked_shared_lock_path_is_refused() -> Checked {
    let dir = temp_dir()?;
    let real = dir.path().join("real.lock");
    fs::write(&real, b"").required()?;
    let link = dir.path().join("session.lock");
    std::os::unix::fs::symlink(&real, &link).required()?;

    let error = acquire_shared_lock(
        &link,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("symlinked lock paths are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    Ok(())
}

#[test]
fn a_shared_lock_file_survives_the_workflow_that_created_it() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("session.lock");
    {
        let _lock = acquire_shared_lock(
            &path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
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
fn a_stale_shared_lock_file_alone_does_not_block_a_fresh_holder() -> Checked {
    // fileの存在だけでは保持中とみなさない。中身が残っていても、OS lockを誰も
    // 保持していなければ、新しいholderは待たされない。
    let dir = temp_dir()?;
    let path = dir.path().join("session.lock");
    fs::write(&path, b"garbage").required_because("leave a stale file behind")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
        .required_because("match the mode a real lock file would have")?;

    thread::scope(|scope| -> Checked {
        let handle = scope.spawn(|| {
            acquire_shared_lock(
                &path,
                LOCK_TIMEOUT,
                PRIVATE_FILE_MODE,
                PathScope::ProjectPath,
            )
        });
        handle
            .join()
            .required_because("thread joins")?
            .required_because("a stale file with no OS lock held is acquired immediately")?;
        Ok(())
    })?;
    Ok(())
}
