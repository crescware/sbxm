//! 契約test: 実OSのfilesystemで、PATH上の実行ファイルの実体を観測できること。
//!
//! 実行可能fileは実行時に書かない。repositoryにある`tests/fixtures/fake_tool.sh`と、
//! 置いてある`/bin/sh`を別の名前で指す。

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::testing::fs::temp_dir;
use crate::testing::outcome::{Checked, Required};

use super::{inspect_executable_at, inspect_executable_on_path, locate_in_path_value};

fn script_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_tool.sh")
}

#[test]
fn a_symlink_is_resolved_to_its_target_and_described_by_the_targets_metadata() -> Checked {
    let dir = temp_dir()?;
    let link = dir.path().join("sbx");
    std::os::unix::fs::symlink("/bin/sh", &link).required_because("link the executable")?;

    let executable = inspect_executable_at(&link).required_because("a linked executable")?;
    let target = std::fs::canonicalize("/bin/sh").required()?;
    let metadata = std::fs::metadata(&target).required()?;
    assert_eq!(executable.identity.path, target);
    assert_eq!(executable.identity.device, metadata.dev());
    assert_eq!(executable.identity.inode, metadata.ino());
    assert_eq!(executable.identity.size, metadata.size());
    assert_eq!(executable.identity.mtime_secs, metadata.mtime());
    assert_eq!(executable.identity.mtime_nanos, metadata.mtime_nsec());
    assert_eq!(executable.identity.ctime_secs, metadata.ctime());
    assert_eq!(executable.identity.ctime_nanos, metadata.ctime_nsec());
    assert!(!executable.script, "/bin/sh is a binary, not a script");
    Ok(())
}

#[test]
fn a_file_starting_with_a_shebang_is_a_script() -> Checked {
    let executable =
        inspect_executable_at(&script_fixture()).required_because("the fixture is executable")?;
    assert!(executable.script);
    assert_eq!(
        executable.identity.path,
        std::fs::canonicalize(script_fixture()).required()?
    );
    Ok(())
}

#[test]
fn what_is_not_an_executable_file_is_not_observed() -> Checked {
    let dir = temp_dir()?;
    assert!(inspect_executable_at(&dir.path().join("missing")).is_none());
    assert!(
        inspect_executable_at(dir.path()).is_none(),
        "a directory is not an executable file"
    );
    let dangling = dir.path().join("dangling");
    std::os::unix::fs::symlink(dir.path().join("gone"), &dangling).required()?;
    assert!(inspect_executable_at(&dangling).is_none());
    Ok(())
}

#[test]
fn path_lookup_observes_the_first_executable_and_skips_what_cannot_run() -> Checked {
    let first = temp_dir()?;
    let second = temp_dir()?;
    // 先頭のdirectoryにある同名のfileは実行bitを持たず、起動されない。
    std::fs::write(first.path().join("sbxm-fake-tool"), "data").required()?;
    std::os::unix::fs::symlink(script_fixture(), second.path().join("sbxm-fake-tool"))
        .required()?;
    let joined = std::env::join_paths([first.path(), second.path()]).required()?;

    let located = locate_in_path_value("sbxm-fake-tool", &joined).required()?;
    assert_eq!(located, second.path().join("sbxm-fake-tool"));
    let executable = inspect_executable_at(&located).required()?;
    assert!(executable.script);

    assert!(inspect_executable_on_path("sbxm-no-such-program").is_none());
    Ok(())
}
