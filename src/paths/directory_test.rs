use super::*;
use crate::error::ErrorId;
use crate::paths::PRIVATE_DIR_MODE;
use crate::paths::inspect::current_user;
use crate::testing::fs::temp_dir;

#[test]
fn private_dir_is_created_with_the_requested_mode() {
    let dir = temp_dir();
    let target = dir.path().join("sbxm");
    ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ConfigDir).expect("create");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, PRIVATE_DIR_MODE);
}

#[test]
fn private_dir_refuses_an_over_permissive_existing_directory() {
    let dir = temp_dir();
    let target = dir.path().join("sbxm");
    fs::create_dir(&target).expect("create");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o777)).expect("widen");

    let error = ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ConfigDir)
        .expect_err("an open directory is not repaired automatically");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigDirPermissionTooOpen));
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o777, "sbxm must not repair permissions on its own");
}

#[test]
fn private_dir_refuses_a_symlink() {
    let dir = temp_dir();
    let real = dir.path().join("real");
    fs::create_dir(&real).expect("create");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");

    let error = ensure_private_dir(&link, PRIVATE_DIR_MODE, PathScope::ConfigDir)
        .expect_err("symlinked directories are refused");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigDirSymlink));
}

#[test]
fn a_private_directory_the_current_user_owns_is_accepted() {
    let dir = temp_dir();
    let target = dir.path().join("sbxm");
    ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ProjectPath).expect("create");
    assert_eq!(
        fs::symlink_metadata(&target).unwrap().uid(),
        current_user(),
        "a directory sbxm creates belongs to the user who ran it"
    );
    ensure_private_dir(&target, PRIVATE_DIR_MODE, PathScope::ProjectPath)
        .expect("a directory the user owns is reused");
}

#[test]
fn a_directory_is_created_once_and_reused_afterwards() {
    let dir = temp_dir();
    let target = dir.path().join("owner").join("repo.project");
    ensure_directory(&target).expect("create");
    assert!(target.is_dir());
    fs::write(target.join("marker"), b"kept").expect("write marker");

    ensure_directory(&target).expect("an existing directory is reused");
    assert_eq!(
        fs::read_to_string(target.join("marker")).unwrap(),
        "kept",
        "an existing directory must not be recreated"
    );
}

#[test]
fn a_directory_is_never_created_through_a_symlink_or_over_another_file() {
    let dir = temp_dir();
    let real = dir.path().join("real");
    fs::create_dir(&real).expect("create");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let error = ensure_directory(&link).expect_err("symlinks are refused");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));

    let file = dir.path().join("file");
    fs::write(&file, b"x").expect("write");
    let error = ensure_directory(&file).expect_err("an existing file is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    assert_eq!(fs::read_to_string(&file).unwrap(), "x");
}

#[test]
fn a_private_directory_is_never_observed_wider_than_it_was_asked_for() {
    // 作ってからpermissionを絞ると、そのあいだに別のprocessが広いmodeを観測する。
    // mkdirの時点でmodeを決めることで、途中の状態を見せない。
    let dir = temp_dir();
    let target = dir.path().join("state");
    let observed: Vec<Result<()>> = std::thread::scope(|scope| {
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
            .map(|handle| handle.join().expect("the thread finishes"))
            .collect()
    });

    for outcome in observed {
        outcome.expect("every run sees a directory it may use");
    }
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        PRIVATE_DIR_MODE
    );
}
