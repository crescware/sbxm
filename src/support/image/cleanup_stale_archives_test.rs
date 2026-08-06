use std::path::Path;

use crate::paths::ProjectPaths;
use crate::testing::outcome::{Checked, Required};

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
    cleanup_stale_archives(&paths);
    assert!(!paths.cache_dir().exists());
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

    cleanup_stale_archives(&paths);

    assert!(!archive.exists(), "a leftover archive is swept");
    assert!(!temporary.exists(), "a leftover temporary archive is swept");
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

    cleanup_stale_archives(&paths);

    assert!(
        std::fs::symlink_metadata(&link).is_ok(),
        "a symlink is never treated as a stale archive"
    );
    assert!(
        elsewhere.exists(),
        "what the symlink points at is untouched"
    );
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

    cleanup_stale_archives(&paths);

    assert!(
        nested.is_dir(),
        "a directory is never treated as a stale archive"
    );
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

    cleanup_stale_archives(&paths);

    for path in [&too_short, &uppercase, &unrelated, &wrong_extension] {
        assert!(
            path.exists(),
            "{path:?} does not match the stale archive name and is left alone"
        );
    }
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

    cleanup_stale_archives(&paths);

    assert!(
        path.exists(),
        "a name that cannot even be read as UTF-8 is never treated as a stale archive"
    );
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

    cleanup_stale_archives(&paths);
    assert!(!archive.exists());
    // 既に片付いたcacheへもう一度実行しても、何も壊さない。
    cleanup_stale_archives(&paths);
    assert!(!archive.exists());
    Ok(())
}
