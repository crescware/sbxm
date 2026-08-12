use crate::design::Fact;
use crate::diagnostics::ErrorId;
use crate::paths::scope::PathScope;
use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::fs::temp_dir;

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
    assert_eq!(lexically_standardize(Path::new(".")), PathBuf::new());
}

#[test]
fn a_path_that_cannot_be_resolved_is_compared_as_declared() -> Checked {
    let missing = Path::new("/no/such/dir/../file");
    assert_eq!(real_path(missing), PathBuf::from("/no/such/file"));
    assert_eq!(real_path(missing), lexically_standardize(missing));

    let dir = temp_dir()?;
    let real = dir.path().join("real");
    fs::create_dir(&real).required_because("create directory")?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).required_because("create symlink")?;
    assert_eq!(
        real_path(&link),
        fs::canonicalize(&real).required_because("resolve directory")?,
        "an existing path is resolved through its symlinks"
    );
    assert_ne!(real_path(&link), lexically_standardize(&link));
    Ok(())
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
fn a_directory_is_refused_instead_of_being_answered_as_a_regular_file() -> Checked {
    let dir = temp_dir()?;
    let file = dir.path().join("regular");
    fs::write(&file, b"").required_because("write the file")?;
    assert!(regular_file_exists(&file, PathScope::ProjectPath).required()?);
    assert!(
        !regular_file_exists(&dir.path().join("absent"), PathScope::ProjectPath).required()?,
        "a path that is simply not there is not a failure"
    );

    let directory = dir.path().join("a-directory");
    fs::create_dir(&directory).required_because("create the directory")?;
    let error = regular_file_exists(&directory, PathScope::ProjectPath)
        .refused_because("a directory is not a regular file")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    // 何を期待し何を観測したかを示さないと、利用者はどのpathを直せばよいか決められない。
    let args = &error.diagnostics()[0].description.args;
    assert!(
        args.iter()
            .any(|(key, value)| *key == "expected" && value == "regular file"),
        "the expected type is named: {args:?}"
    );
    assert!(
        args.iter()
            .any(|(key, value)| *key == "observed" && value == "directory"),
        "the observed type is named: {args:?}"
    );
    Ok(())
}

#[test]
fn a_directory_that_is_not_there_is_answered_without_a_failure() -> Checked {
    let dir = temp_dir()?;
    let present = dir.path().join("present");
    fs::create_dir(&present).required_because("create the directory")?;
    assert!(directory_exists(&present, PathScope::ProjectPath).required()?);
    assert!(
        !directory_exists(&dir.path().join("absent"), PathScope::ProjectPath).required()?,
        "a directory that is simply not there is not a failure"
    );
    assert!(
        !directory_exists(&dir.path().join("absent/deeper"), PathScope::ProjectPath).required()?,
        "a path under a directory that is not there is absent as well"
    );
    Ok(())
}

#[test]
fn a_path_that_is_not_a_directory_is_refused_instead_of_being_answered() -> Checked {
    let dir = temp_dir()?;
    let file = dir.path().join("regular");
    fs::write(&file, b"").required_because("write the file")?;
    let error = directory_exists(&file, PathScope::ProjectPath)
        .refused_because("a regular file is not a directory")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    let args = &error.diagnostics()[0].description.args;
    assert!(
        args.iter()
            .any(|(key, value)| *key == "expected" && value == "directory"),
        "the expected type is named: {args:?}"
    );

    // 辿った先がdirectoryでも、symlink自体をそのdirectoryとして扱わない。
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(dir.path(), &link).required_because("create the symlink")?;
    let error = directory_exists(&link, PathScope::ProjectPath)
        .refused_because("a symlink is never followed to answer for the path itself")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    Ok(())
}

#[test]
fn a_directory_that_cannot_be_observed_is_never_answered_as_absent() -> Checked {
    let dir = temp_dir()?;
    let closed = dir.path().join("closed");
    fs::create_dir(&closed).required_because("create the parent")?;
    let target = closed.join("workspace");
    fs::create_dir(&target).required_because("create the directory")?;
    fs::set_permissions(&closed, fs::Permissions::from_mode(0o000))
        .required_because("close the parent")?;

    let observed = directory_exists(&target, PathScope::ProjectPath);
    fs::set_permissions(&closed, fs::Permissions::from_mode(0o700))
        .required_because("reopen the parent")?;

    let error =
        observed.refused_because("a directory that cannot be read is never called absent")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    Ok(())
}

#[test]
fn a_directory_opened_as_a_file_is_refused_by_the_private_file_check() -> Checked {
    let dir = temp_dir()?;
    let file = File::open(dir.path()).required_because("open the directory")?;
    let error = require_private_file(
        &file,
        dir.path(),
        crate::paths::PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("a directory is not a private regular file")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    Ok(())
}

#[test]
fn unexpected_types_name_symbolic_links_and_special_files() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("target");
    fs::write(&target, b"").required_because("write the target")?;
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&target, &link).required_because("create the link")?;

    let link_error = unexpected_type(
        &link,
        "regular file",
        &fs::symlink_metadata(&link).required_because("inspect the link")?,
    );
    assert!(
        link_error.diagnostics()[0]
            .description
            .args
            .iter()
            .any(|(key, value)| *key == "observed" && value == "symbolic link")
    );

    let device = Path::new("/dev/null");
    let special_error = unexpected_type(
        device,
        "regular file",
        &fs::metadata(device).required_because("inspect the null device")?,
    );
    assert!(
        special_error.diagnostics()[0]
            .description
            .args
            .iter()
            .any(|(key, value)| *key == "observed" && value == "special file")
    );
    Ok(())
}

#[test]
fn a_path_that_cannot_be_read_is_refused_rather_than_reported_as_absent() -> Checked {
    let dir = temp_dir()?;
    let closed = dir.path().join("closed");
    fs::create_dir(&closed).required_because("create the directory")?;
    let target = closed.join("settings.yaml");
    fs::write(&target, b"").required_because("write the file")?;
    // 親directoryを辿れない間は、fileが在るかどうかそのものを観測できない。
    fs::set_permissions(&closed, fs::Permissions::from_mode(0o000))
        .required_because("close the directory")?;

    let project = regular_file_exists(&target, PathScope::ProjectPath);
    let config = regular_file_exists(&target, PathScope::ConfigFile);
    fs::set_permissions(&closed, fs::Permissions::from_mode(0o700))
        .required_because("reopen the directory")?;

    // 同じ観測でも、案件の成果物とglobal設定では利用者の対処が変わる。
    for (observed, expected) in [
        (project, ErrorId::ProjectPathUnreadable),
        (config, ErrorId::ConfigUnreadable),
    ] {
        let error =
            observed.refused_because("a path that cannot be read is never called absent")?;
        assert_eq!(error.first_id(), Some(expected));
        let facts = &error.diagnostics()[0].facts;
        assert!(
            facts
                .iter()
                .any(|fact| matches!(fact, Fact::OneLine { label, value }
                if label.id == "diagnostic-path-label" && value.as_str() == display(&target))),
            "the path that could not be read is named: {facts:?}"
        );
        assert!(
            facts
                .iter()
                .any(|fact| matches!(fact, Fact::OneLine { label, .. }
                if label.id == "diagnostic-cause-label")),
            "the reason the lookup failed is carried with the diagnostic: {facts:?}"
        );
    }
    Ok(())
}

#[test]
fn a_path_another_account_owns_is_refused_for_the_scope_it_protects() -> Checked {
    // 別accountが所有するpathはtestから作れないため、観測値だけを差し替える。
    let dir = temp_dir()?;
    let target = dir.path().join("owned-by-someone-else");
    fs::create_dir(&target).required_because("create")?;
    let other = current_user().wrapping_add(1);

    for (scope, expected) in [
        (PathScope::ProjectPath, ErrorId::ProjectPathNotOwned),
        (PathScope::ConfigDir, ErrorId::ConfigDirNotOwned),
        (PathScope::ConfigFile, ErrorId::ConfigNotOwned),
    ] {
        let error = require_owned_by_current_user(&target, other, scope)
            .refused_because("a path another account owns is never used")?;
        assert_eq!(error.first_id(), Some(expected));
        let diagnostic = &error.diagnostics()[0];
        assert!(
            diagnostic.remediation.is_some(),
            "{scope:?} must tell the user what to do"
        );
    }

    // 所有者が一致する場合だけ通る。permissionは別の判定である。
    require_owned_by_current_user(&target, current_user(), PathScope::ProjectPath)
        .required_because("the current user owns this path")?;
    Ok(())
}

#[test]
fn a_lock_file_another_account_owns_is_never_taken() -> Checked {
    // 所有者の判定はopenできたかではなく、観測したowner IDで行う。
    let dir = temp_dir()?;
    let path = dir.path().join("project.lock");
    fs::write(&path, b"").required_because("seed the lock file")?;
    fs::set_permissions(
        &path,
        fs::Permissions::from_mode(crate::paths::PRIVATE_FILE_MODE),
    )
    .required_because("mode")?;

    let file = File::open(&path).required_because("open")?;
    require_private_file(
        &file,
        &path,
        crate::paths::PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("a lock file the user owns is usable")?;
    assert_eq!(
        fs::symlink_metadata(&path).required()?.uid(),
        current_user(),
        "the check compares the observed owner with the current user"
    );
    Ok(())
}
