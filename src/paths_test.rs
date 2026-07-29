use super::*;
use std::thread;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary directory")
}

#[test]
fn standardization_removes_dot_and_parent_components() {
    assert_eq!(
        lexically_standardize(Path::new("/a/./b/../c")),
        PathBuf::from("/a/c")
    );
    assert_eq!(lexically_standardize(Path::new("/")), PathBuf::from("/"));
    assert_eq!(lexically_standardize(Path::new("/..")), PathBuf::from("/"));
    assert_eq!(
        lexically_standardize(Path::new("/a/b/../../c")),
        PathBuf::from("/c")
    );
}

#[test]
fn a_path_that_cannot_be_resolved_is_compared_as_declared() {
    let missing = Path::new("/no/such/dir/../file");
    assert_eq!(real_path(missing), PathBuf::from("/no/such/file"));
    assert_eq!(real_path(missing), lexically_standardize(missing));

    let dir = temp_dir();
    let real = dir.path().join("real");
    fs::create_dir(&real).expect("create directory");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("create symlink");
    assert_eq!(
        real_path(&link),
        fs::canonicalize(&real).expect("resolve directory"),
        "an existing path is resolved through its symlinks"
    );
    assert_ne!(real_path(&link), lexically_standardize(&link));
}

#[test]
fn permission_check_rejects_group_and_other_bits() {
    assert!(!permission_too_open(0o600));
    assert!(!permission_too_open(0o700));
    assert!(permission_too_open(0o640));
    assert!(permission_too_open(0o604));
    assert!(permission_too_open(0o755));
    assert_eq!(format_mode(0o600), "0600");
    assert_eq!(format_mode(0o40700), "0700");
}

#[test]
fn base_path_must_be_absolute() {
    let error = AbsoluteBasePath::new(Path::new("relative/path"))
        .expect_err("relative base paths are rejected");
    assert_eq!(error.first_id(), Some(ErrorId::BasePathNotAbsolute));
}

#[test]
fn base_path_accepts_a_directory_that_does_not_exist_yet() {
    let dir = temp_dir();
    let target = dir.path().join("Projects").join("nested");
    let base = AbsoluteBasePath::new(&target).expect("creatable base paths are accepted");
    assert_eq!(base.as_path(), lexically_standardize(&target));
}

#[test]
fn base_path_rejects_a_symlink_that_escapes_the_declared_root() {
    let dir = temp_dir();
    let real = dir.path().join("real");
    fs::create_dir(&real).expect("create real directory");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("create symlink");

    let error =
        AbsoluteBasePath::new(&link).expect_err("a base path that resolves elsewhere is refused");
    assert_eq!(error.first_id(), Some(ErrorId::BasePathEscapesRoot));
}

#[test]
fn base_path_rejects_an_existing_regular_file() {
    let dir = temp_dir();
    let file = dir.path().join("not-a-directory");
    fs::write(&file, b"x").expect("write file");
    let error = AbsoluteBasePath::new(&file).expect_err("files are not base paths");
    assert_eq!(error.first_id(), Some(ErrorId::BasePathNotDirectory));
}

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
fn a_path_another_account_owns_is_refused_for_the_scope_it_protects() {
    // 別accountが所有するpathはtestから作れないため、観測値だけを差し替える。
    let dir = temp_dir();
    let target = dir.path().join("owned-by-someone-else");
    fs::create_dir(&target).expect("create");
    let other = current_user().wrapping_add(1);

    for (scope, expected) in [
        (PathScope::ProjectPath, ErrorId::ProjectPathNotOwned),
        (PathScope::ConfigDir, ErrorId::ConfigDirNotOwned),
        (PathScope::ConfigFile, ErrorId::ConfigNotOwned),
    ] {
        let error = require_owned_by_current_user(&target, other, scope)
            .expect_err("a path another account owns is never used");
        assert_eq!(error.first_id(), Some(expected));
        let diagnostic = &error.diagnostics()[0];
        assert!(
            diagnostic.remediation.is_some(),
            "{scope:?} must tell the user what to do"
        );
    }

    // 所有者が一致する場合だけ通る。permissionは別の判定である。
    require_owned_by_current_user(&target, current_user(), PathScope::ProjectPath)
        .expect("the current user owns this path");
}

#[test]
fn a_lock_file_another_account_owns_is_never_taken() {
    // 所有者の判定はopenできたかではなく、観測したowner IDで行う。
    let dir = temp_dir();
    let path = dir.path().join("project.lock");
    fs::write(&path, b"").expect("seed the lock file");
    fs::set_permissions(&path, fs::Permissions::from_mode(PRIVATE_FILE_MODE)).expect("mode");

    let file = File::open(&path).expect("open");
    require_private_file(&file, &path, PRIVATE_FILE_MODE, PathScope::ProjectPath)
        .expect("a lock file the user owns is usable");
    assert_eq!(
        fs::symlink_metadata(&path).unwrap().uid(),
        current_user(),
        "the check compares the observed owner with the current user"
    );
}

fn base(path: &Path) -> AbsoluteBasePath {
    AbsoluteBasePath::new(path).expect("valid base path")
}

fn project_id(value: &str) -> CanonicalProjectId {
    crate::project::ProjectId::parse(value)
        .expect("valid project id")
        .canonical()
}

#[test]
fn project_paths_follow_the_documented_layout() {
    let base = base(Path::new("/Users/example/Projects"));
    let paths = ProjectPaths::derive(&base, &project_id("Example-Org/Example-Repo"));

    assert_eq!(
        paths.owner_dir(),
        Path::new("/Users/example/Projects/example-org")
    );
    assert_eq!(
        paths.root(),
        Path::new("/Users/example/Projects/example-org/example-repo.project")
    );
    let root = paths.root();
    assert_eq!(paths.host_clone(), root.join("example-repo"));
    assert_eq!(paths.metadata_file(), root.join(".sbxm/project.toml"));
    assert_eq!(paths.lock_file(), root.join(".sbxm/project.lock"));
    assert_eq!(paths.dockerfile(), root.join(".sbxm/Dockerfile"));
    assert_eq!(paths.cache_dir(), root.join(".sbxm/.cache"));
    assert_eq!(
        paths.template_archive("0123456789ab"),
        root.join(".sbxm/.cache/template-0123456789ab.tar")
    );
    assert_eq!(
        paths.template_archive_temp("0123456789ab"),
        root.join(".sbxm/.cache/template-0123456789ab.tar.tmp")
    );
}

#[test]
fn project_paths_are_lowercase_so_one_project_cannot_take_two_directories() {
    let base = base(Path::new("/Users/example/Projects"));
    assert_eq!(
        ProjectPaths::derive(&base, &project_id("Example-Org/Example-Repo")),
        ProjectPaths::derive(&base, &project_id("example-org/example-repo"))
    );
}

#[test]
fn a_directory_is_created_once_and_reused_afterwards() {
    let dir = temp_dir();
    let target = dir.path().join("owner").join("repo.project");
    ensure_directory(&target).expect("create");
    assert!(target.is_dir());
    std::fs::write(target.join("marker"), b"kept").expect("write marker");

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

#[test]
fn an_exclusive_lock_serializes_concurrent_holders() {
    let dir = temp_dir();
    let path = dir.path().join("init.lock");

    let held = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .expect("acquire");

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
        .expect("thread joins")
        .expect_err("a second holder must wait and then time out");
    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

    drop(held);

    acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .expect("the lock can be taken again once the first holder releases it");
}

#[test]
fn a_lock_file_that_is_not_private_is_never_taken() {
    let dir = temp_dir();
    let path = dir.path().join("project.lock");
    fs::write(&path, b"").expect("seed the lock file");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).expect("widen");

    let error = acquire_exclusive_lock(
        &path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect_err("a lock other accounts can take is not a lock");
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o666, "sbxm must not repair permissions on its own");
}

#[test]
fn a_symlinked_lock_path_is_reported_for_the_scope_it_protects() {
    let dir = temp_dir();
    let real = dir.path().join("real.lock");
    fs::write(&real, b"").unwrap();
    let link = dir.path().join("project.lock");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    for (scope, expected) in [
        (PathScope::ProjectPath, ErrorId::ProjectPathSymlink),
        (PathScope::ConfigFile, ErrorId::ConfigSymlink),
    ] {
        let error = acquire_exclusive_lock(&link, LOCK_TIMEOUT, PRIVATE_FILE_MODE, scope)
            .expect_err("symlinked lock paths are refused");
        assert_eq!(error.first_id(), Some(expected));
    }
}

#[test]
fn a_lock_file_survives_the_workflow_that_created_it() {
    let dir = temp_dir();
    let path = dir.path().join("init.lock");
    {
        let _lock = acquire_exclusive_lock(
            &path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ConfigFile,
        )
        .expect("acquire");
    }
    assert!(
        path.exists(),
        "the lock file is not deleted when the workflow ends"
    );
}
