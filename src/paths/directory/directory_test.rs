use crate::paths::scope::PathScope;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;
use crate::paths::PRIVATE_DIR_MODE;
use crate::paths::inspect::current_user;
use crate::testing::fs::temp_dir;

#[test]
fn private_dir_is_created_with_the_requested_mode() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("sbxm");
    ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ConfigDir)
        .required_because("create")?;
    let mode = fs::metadata(&target).required()?.permissions().mode() & 0o777;
    assert_eq!(mode, PRIVATE_DIR_MODE);
    Ok(())
}

#[test]
fn private_dir_refuses_an_over_permissive_existing_directory() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("sbxm");
    fs::create_dir(&target).required_because("create")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o777)).required_because("widen")?;

    let error = ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ConfigDir)
        .refused_because("an open directory is not repaired automatically")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigDirPermissionTooOpen));
    let mode = fs::metadata(&target).required()?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o777, "sbxm must not repair permissions on its own");
    Ok(())
}

#[test]
fn private_dir_refuses_a_symlink() -> Checked {
    let dir = temp_dir()?;
    let real = dir.path().join("real");
    fs::create_dir(&real).required_because("create")?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).required_because("symlink")?;

    let error = ensure_private_dir(&link, PRIVATE_DIR_MODE, PathScope::ConfigDir)
        .refused_because("symlinked directories are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigDirSymlink));
    Ok(())
}

#[test]
fn a_private_directory_the_current_user_owns_is_accepted() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("sbxm");
    ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ProjectPath)
        .required_because("create")?;
    assert_eq!(
        fs::symlink_metadata(&target).required()?.uid(),
        current_user(),
        "a directory sbxm creates belongs to the user who ran it"
    );
    ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ProjectPath)
        .required_because("a directory the user owns is reused")?;
    Ok(())
}

#[test]
fn a_directory_is_created_once_and_reused_afterwards() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("owner").join("repo.project");
    ensure_directory(&target).required_because("create")?;
    assert!(target.is_dir());
    fs::write(target.join("marker"), b"kept").required_because("write marker")?;

    ensure_directory(&target).required_because("an existing directory is reused")?;
    assert_eq!(
        fs::read_to_string(target.join("marker")).required()?,
        "kept",
        "an existing directory must not be recreated"
    );
    Ok(())
}

#[test]
fn a_directory_is_never_created_through_a_symlink_or_over_another_file() -> Checked {
    let dir = temp_dir()?;
    let real = dir.path().join("real");
    fs::create_dir(&real).required_because("create")?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).required_because("symlink")?;
    let error = ensure_directory(&link).refused_because("symlinks are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));

    let file = dir.path().join("file");
    fs::write(&file, b"x").required_because("write")?;
    let error = ensure_directory(&file).refused_because("an existing file is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    assert_eq!(fs::read_to_string(&file).required()?, "x");
    Ok(())
}

#[test]
fn a_private_directory_is_never_observed_wider_than_it_was_asked_for() -> Checked {
    // 作ってからpermissionを絞ると、そのあいだに別のprocessが広いmodeを観測する。
    // mkdirの時点でmodeを決めることで、途中の状態を見せない。
    let dir = temp_dir()?;
    let target = dir.path().join("state");
    let observed = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let target = target.clone();
                scope.spawn(move || {
                    ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ConfigDir)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(std::thread::ScopedJoinHandle::join)
            .collect::<Vec<_>>()
    });

    for outcome in observed {
        outcome
            .required_because("the thread finishes")?
            .required_because("every run sees a directory it may use")?;
    }
    assert_eq!(
        fs::metadata(&target).required()?.permissions().mode() & 0o777,
        PRIVATE_DIR_MODE
    );
    Ok(())
}
