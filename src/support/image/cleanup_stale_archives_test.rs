use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::diagnostics::ErrorId;
use crate::paths::ProjectPaths;
use crate::testing::outcome::{Checked, Refused, Required};

use super::{cleanup_stale_archives, fake::canonical};

fn project_paths(dir: &Path) -> Checked<ProjectPaths> {
    let base = crate::paths::ProjectParent::at(dir).required_because("valid parent directory")?;
    Ok(ProjectPaths::derive(&base, &canonical()?))
}

#[test]
fn a_missing_cache_directory_is_a_no_op() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    // `.cache`を一度も作っていない、登録直後の状態を模す。
    let warnings = cleanup_stale_archives(&paths).required()?;
    assert!(!paths.cache_dir().exists());
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

#[test]
fn exact_match_regular_files_are_removed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    let hex = "0".repeat(12);
    let archive = paths.cache_dir().join(format!("template-{hex}.tar"));
    let temporary = paths.cache_dir().join(format!("template-{hex}.tar.tmp"));
    std::fs::write(&archive, b"a stale archive").required()?;
    std::fs::write(&temporary, b"a stale temporary archive").required()?;

    let warnings = cleanup_stale_archives(&paths).required()?;

    assert!(!archive.exists(), "a leftover archive is swept");
    assert!(!temporary.exists(), "a leftover temporary archive is swept");
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

#[test]
fn a_symlink_named_like_an_archive_is_left_alone() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    let elsewhere = dir.path().join("elsewhere.tar");
    std::fs::write(&elsewhere, b"not a stale archive").required()?;
    let hex = "0".repeat(12);
    let link = paths.cache_dir().join(format!("template-{hex}.tar"));
    std::os::unix::fs::symlink(&elsewhere, &link).required()?;

    let warnings = cleanup_stale_archives(&paths).required()?;

    assert!(
        std::fs::symlink_metadata(&link).is_ok(),
        "a symlink is never treated as a stale archive"
    );
    assert!(
        elsewhere.exists(),
        "what the symlink points at is untouched"
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

#[test]
fn a_directory_named_like_an_archive_is_left_alone() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    let hex = "0".repeat(12);
    let nested = paths.cache_dir().join(format!("template-{hex}.tar"));
    std::fs::create_dir(&nested).required()?;

    let warnings = cleanup_stale_archives(&paths).required()?;

    assert!(
        nested.is_dir(),
        "a directory is never treated as a stale archive"
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

#[test]
fn names_that_do_not_match_the_stale_archive_pattern_are_left_alone() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    let too_short = paths.cache_dir().join("template-abc.tar");
    let uppercase = paths
        .cache_dir()
        .join(format!("template-{}.tar", "A".repeat(12)));
    let unrelated = paths.cache_dir().join("notes.txt");
    let wrong_extension = paths
        .cache_dir()
        .join(format!("template-{}.zip", "0".repeat(12)));
    for path in [&too_short, &uppercase, &unrelated, &wrong_extension] {
        std::fs::write(path, b"left alone").required()?;
    }

    let warnings = cleanup_stale_archives(&paths).required()?;

    for path in [&too_short, &uppercase, &unrelated, &wrong_extension] {
        assert!(
            path.exists(),
            "{path:?} does not match the stale archive name and is left alone"
        );
    }
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

#[test]
fn a_name_that_is_not_valid_utf8_is_left_alone() -> Checked {
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    // 有効なUTF-8にはならない、単独の続きbyte。
    let mut invalid = b"template-".to_vec();
    invalid.extend_from_slice(&[0x80, 0x81]);
    invalid.extend_from_slice(b".tar");
    let name = std::ffi::OsStr::from_bytes(&invalid);
    let path = paths.cache_dir().join(name);
    std::fs::write(&path, b"left alone").required()?;

    let warnings = cleanup_stale_archives(&paths).required()?;

    assert!(
        path.exists(),
        "a name that cannot even be read as UTF-8 is never treated as a stale archive"
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    Ok(())
}

#[test]
fn running_it_twice_ends_in_the_same_state() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    let archive = paths
        .cache_dir()
        .join(format!("template-{}.tar", "0".repeat(12)));
    std::fs::write(&archive, b"a stale archive").required()?;

    cleanup_stale_archives(&paths).required()?;
    assert!(!archive.exists());
    // 既に片付いたcacheへもう一度実行しても、何も壊さない。
    cleanup_stale_archives(&paths).required()?;
    assert!(!archive.exists());
    Ok(())
}

#[test]
fn a_symlinked_cache_directory_is_refused_instead_of_traversed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.sbxm_dir()).required()?;
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).required()?;
    let hex = "0".repeat(12);
    let outside = elsewhere.join(format!("template-{hex}.tar"));
    std::fs::write(&outside, b"not part of this project").required()?;
    std::os::unix::fs::symlink(&elsewhere, paths.cache_dir()).required()?;

    let error = cleanup_stale_archives(&paths)
        .refused_because("a symlinked cache directory is never traversed")?;

    assert!(error.contains_id(ErrorId::ProjectPathSymlink), "{error:?}");
    assert!(
        outside.exists(),
        "a file outside the project is never touched through a symlinked cache directory"
    );
    Ok(())
}

#[test]
fn an_unreadable_cache_directory_is_reported_as_a_warning() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    std::fs::set_permissions(paths.cache_dir(), std::fs::Permissions::from_mode(0o000))
        .required()?;

    let outcome = cleanup_stale_archives(&paths);

    std::fs::set_permissions(paths.cache_dir(), std::fs::Permissions::from_mode(0o700))
        .required()?;

    let warnings =
        outcome.required_because("an unreadable cache is reported, not silently ignored")?;
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(
        warnings[0].description.id,
        "warning-stale-archive-unreadable"
    );
    Ok(())
}

#[test]
fn a_stale_archive_that_cannot_be_removed_is_reported_as_a_warning() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    std::fs::create_dir_all(paths.cache_dir()).required()?;
    let hex = "0".repeat(12);
    let archive = paths.cache_dir().join(format!("template-{hex}.tar"));
    std::fs::write(&archive, b"a stale archive").required()?;
    // 消す権利はfile自身ではなく、それを載せているdirectoryが持つ。
    std::fs::set_permissions(paths.cache_dir(), std::fs::Permissions::from_mode(0o500))
        .required()?;

    let outcome = cleanup_stale_archives(&paths);

    std::fs::set_permissions(paths.cache_dir(), std::fs::Permissions::from_mode(0o700))
        .required()?;

    let warnings =
        outcome.required_because("a removal failure is reported, not silently ignored")?;
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(
        warnings[0].description.id,
        "warning-stale-archive-cleanup-failed"
    );
    assert!(
        archive.exists(),
        "the archive that could not be removed is left in place"
    );
    Ok(())
}
