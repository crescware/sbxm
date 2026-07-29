use super::*;
use crate::paths::PRIVATE_FILE_MODE;
use crate::testing::fs::temp_dir;

#[test]
fn atomic_create_writes_the_requested_mode_and_content() {
    let dir = temp_dir();
    let target = dir.path().join("config.toml");
    atomic_create(&target, "version = 1\n", PRIVATE_FILE_MODE).expect("atomic create");

    assert_eq!(fs::read_to_string(&target).unwrap(), "version = 1\n");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, PRIVATE_FILE_MODE);
    assert!(
        !dir.path().join(".config.toml.tmp").exists(),
        "the temporary file must not survive a successful write"
    );
}

#[test]
fn atomic_create_refuses_to_overwrite_a_target_that_appeared() {
    let dir = temp_dir();
    let target = dir.path().join("config.toml");
    fs::write(&target, "existing").expect("seed target");

    let error = atomic_create(&target, "replacement", PRIVATE_FILE_MODE)
        .expect_err("an existing target must not be overwritten");
    assert_eq!(error.first_id(), Some(ErrorId::TargetAppearedConcurrently));
    assert_eq!(fs::read_to_string(&target).unwrap(), "existing");
    assert!(!dir.path().join(".config.toml.tmp").exists());
}

#[test]
fn an_interrupted_temporary_file_is_reported_instead_of_deleted() {
    let dir = temp_dir();
    let target = dir.path().join("config.toml");
    let temp = dir.path().join(".config.toml.tmp");
    fs::write(&temp, "interrupted").expect("seed temporary file");

    let error = atomic_create(&target, "new", PRIVATE_FILE_MODE)
        .expect_err("a leftover temporary file stops the write");
    assert_eq!(error.first_id(), Some(ErrorId::TempFileLeftBehind));
    assert_eq!(
        fs::read_to_string(&temp).unwrap(),
        "interrupted",
        "the leftover file must be preserved for inspection"
    );
    assert!(!target.exists());
}

#[test]
fn atomic_replace_swaps_the_content_and_keeps_the_requested_mode() {
    let dir = temp_dir();
    let target = dir.path().join("project.toml");
    atomic_create(&target, "version = 1\n", PRIVATE_FILE_MODE).expect("create");

    atomic_replace(
        &target,
        "version = 1\nstart_ref = \"main\"\n",
        PRIVATE_FILE_MODE,
    )
    .expect("replace");

    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "version = 1\nstart_ref = \"main\"\n"
    );
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, PRIVATE_FILE_MODE);
    assert!(!dir.path().join(".project.toml.tmp").exists());
}

#[test]
fn atomic_replace_refuses_a_symlink_a_directory_and_an_open_file() {
    let dir = temp_dir();

    let real = dir.path().join("real.toml");
    fs::write(&real, "version = 1\n").unwrap();
    let link = dir.path().join("link.toml");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let error = atomic_replace(&link, "replaced", PRIVATE_FILE_MODE)
        .expect_err("symlinked targets are refused");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert_eq!(fs::read_to_string(&real).unwrap(), "version = 1\n");

    let directory = dir.path().join("a-directory");
    fs::create_dir(&directory).unwrap();
    let error = atomic_replace(&directory, "replaced", PRIVATE_FILE_MODE)
        .expect_err("directories are refused");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));

    let shared = dir.path().join("shared.toml");
    fs::write(&shared, "version = 1\n").unwrap();
    fs::set_permissions(&shared, fs::Permissions::from_mode(0o666)).unwrap();
    let error = atomic_replace(&shared, "replaced", PRIVATE_FILE_MODE)
        .expect_err("a world-writable target is refused");
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert_eq!(fs::read_to_string(&shared).unwrap(), "version = 1\n");
}

#[test]
fn atomic_replace_leaves_a_target_that_became_a_different_file_alone() {
    let dir = temp_dir();
    let target = dir.path().join("project.toml");
    atomic_create(&target, "first\n", PRIVATE_FILE_MODE).expect("create");
    let original = FileIdentity::of_path_without_following(&target).unwrap();

    // 書いている間に別のprocessがfileを作り直した状況を作る。
    let replacement = dir.path().join("other.toml");
    atomic_create(&replacement, "second\n", PRIVATE_FILE_MODE).expect("create");
    fs::rename(&replacement, &target).expect("swap the target");
    assert_ne!(
        FileIdentity::of_path_without_following(&target).unwrap(),
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
        .expect_err("a target that changed identity is not overwritten");
    assert_eq!(error.first_id(), Some(ErrorId::TargetChangedConcurrently));
    assert_eq!(fs::read_to_string(&target).unwrap(), "second\n");
    assert!(!dir.path().join(".project.toml.tmp").exists());
}
