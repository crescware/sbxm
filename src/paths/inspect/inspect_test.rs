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
