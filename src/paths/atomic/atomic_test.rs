use crate::diagnostics::{ErrorId, fail};
use crate::msg;
use crate::paths::inspect::{FileIdentity, display};
use std::fs::{self};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::paths::PRIVATE_FILE_MODE;
use crate::testing::fs::temp_dir;

#[test]
fn atomic_create_writes_the_requested_mode_and_content() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("config.yaml");
    atomic_create(&target, "version: 1\n", PRIVATE_FILE_MODE).required_because("atomic create")?;

    assert_eq!(fs::read_to_string(&target).required()?, "version: 1\n");
    let mode = fs::metadata(&target).required()?.permissions().mode() & 0o777;
    assert_eq!(mode, PRIVATE_FILE_MODE);
    assert!(
        !dir.path().join(".config.yaml.tmp").exists(),
        "the temporary file must not survive a successful write"
    );
    Ok(())
}

#[test]
fn atomic_create_refuses_to_overwrite_a_target_that_appeared() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("config.yaml");
    fs::write(&target, "existing").required_because("seed target")?;

    let error = atomic_create(&target, "replacement", PRIVATE_FILE_MODE)
        .refused_because("an existing target must not be overwritten")?;
    assert_eq!(error.first_id(), Some(ErrorId::TargetAppearedConcurrently));
    assert_eq!(fs::read_to_string(&target).required()?, "existing");
    assert!(!dir.path().join(".config.yaml.tmp").exists());
    Ok(())
}

#[test]
fn an_interrupted_temporary_file_is_reported_instead_of_deleted() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("config.yaml");
    let temp = dir.path().join(".config.yaml.tmp");
    fs::write(&temp, "interrupted").required_because("seed temporary file")?;

    let error = atomic_create(&target, "new", PRIVATE_FILE_MODE)
        .refused_because("a leftover temporary file stops the write")?;
    assert_eq!(error.first_id(), Some(ErrorId::TempFileLeftBehind));
    assert_eq!(
        fs::read_to_string(&temp).required()?,
        "interrupted",
        "the leftover file must be preserved for inspection"
    );
    assert!(!target.exists());
    Ok(())
}

#[test]
fn atomic_replace_swaps_the_content_and_keeps_the_requested_mode() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("project.yaml");
    atomic_create(&target, "version: 1\n", PRIVATE_FILE_MODE).required_because("create")?;

    atomic_replace(&target, "version: 1\nstart_ref: main\n", PRIVATE_FILE_MODE)
        .required_because("replace")?;

    assert_eq!(
        fs::read_to_string(&target).required()?,
        "version: 1\nstart_ref: main\n"
    );
    let mode = fs::metadata(&target).required()?.permissions().mode() & 0o777;
    assert_eq!(mode, PRIVATE_FILE_MODE);
    assert!(!dir.path().join(".project.yaml.tmp").exists());
    Ok(())
}

#[test]
fn atomic_replace_refuses_a_symlink_a_directory_and_an_open_file() -> Checked {
    let dir = temp_dir()?;

    let real = dir.path().join("real.yaml");
    fs::write(&real, "version: 1\n").required()?;
    let link = dir.path().join("link.yaml");
    std::os::unix::fs::symlink(&real, &link).required()?;
    let error = atomic_replace(&link, "replaced", PRIVATE_FILE_MODE)
        .refused_because("symlinked targets are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert_eq!(fs::read_to_string(&real).required()?, "version: 1\n");

    let directory = dir.path().join("a-directory");
    fs::create_dir(&directory).required()?;
    let error = atomic_replace(&directory, "replaced", PRIVATE_FILE_MODE)
        .refused_because("directories are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));

    let shared = dir.path().join("shared.yaml");
    fs::write(&shared, "version: 1\n").required()?;
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o666)).required()?;
    let error = atomic_replace(&shared, "replaced", PRIVATE_FILE_MODE)
        .refused_because("a world-writable target is refused")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert_eq!(fs::read_to_string(&shared).required()?, "version: 1\n");
    Ok(())
}

#[test]
fn atomic_replace_leaves_a_target_that_became_a_different_file_alone() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("project.yaml");
    atomic_create(&target, "first\n", PRIVATE_FILE_MODE).required_because("create")?;
    let original = FileIdentity::of_path_without_following(&target).required()?;

    // 書いている間に別のprocessがfileを作り直した状況を作る。
    let replacement = dir.path().join("other.yaml");
    atomic_create(&replacement, "second\n", PRIVATE_FILE_MODE).required_because("create")?;
    fs::rename(&replacement, &target).required_because("swap the target")?;
    assert_ne!(
        FileIdentity::of_path_without_following(&target).required()?,
        original
    );

    // 置き換えの直前に再取得したidentityが一致することは、この時点では検査済みである。
    // 検査後に差し替わる状況をprecondition側で確認する。
    let error =
        atomic_write_with_precondition(&target, "third\n", PRIVATE_FILE_MODE, |target: &Path| {
            let observed = replaceable_identity(target, PRIVATE_FILE_MODE)?;
            if observed != original {
                return fail(
                    ErrorId::TargetChangedConcurrently,
                    msg!("error-target-changed-concurrently", path = display(target)),
                );
            }
            Ok(())
        })
        .refused_because("a target that changed identity is not overwritten")?;
    assert_eq!(error.first_id(), Some(ErrorId::TargetChangedConcurrently));
    assert_eq!(fs::read_to_string(&target).required()?, "second\n");
    assert!(!dir.path().join(".project.yaml.tmp").exists());
    Ok(())
}
