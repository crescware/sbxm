use crate::design::Fact;
use crate::diagnostics::{Error, ErrorId};
use crate::paths::inspect::FileIdentity;
use std::fs::{self};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::paths::PRIVATE_FILE_MODE;
use crate::testing::fs::temp_dir;

/// pathが無いことをOSが述べる原文。綴りはOSが決めるため、同じerrnoから組み立てる。
fn no_such_file() -> String {
    std::io::Error::from_raw_os_error(2).to_string()
}

/// 診断が挙げた事実のうち、sbxm自身が観測した原因のmessage ID。
fn reason_of(error: &Error) -> Checked<&'static str> {
    error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::Translated { value, .. } => Some(value.id),
            _ => None,
        })
        .required_because("the observed reason is named")
}

/// 診断が挙げた事実のうち、OSが書いた原文。
fn cause_of(error: &Error) -> Checked<String> {
    error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-cause-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the cause is quoted from the operating system")
}

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
fn a_temporary_file_that_cannot_be_created_reports_what_the_operating_system_said() -> Checked {
    // `AlreadyExists`ではない失敗（parent directoryが書き込みを許さない、など）は、
    // 中断の跡としてではなく、そのまま書き込みの失敗として報告する。
    let dir = temp_dir()?;
    let closed = dir.path().join("closed");
    fs::create_dir(&closed).required_because("create the parent")?;
    let target = closed.join("config.yaml");

    fs::set_permissions(&closed, fs::Permissions::from_mode(0o500))
        .required_because("close the parent directory to writes")?;
    let outcome = atomic_create(&target, "version: 1\n", PRIVATE_FILE_MODE);
    fs::set_permissions(&closed, fs::Permissions::from_mode(0o700))
        .required_because("reopen the parent directory")?;

    let error = outcome.refused_because("a temporary file that cannot be created is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert!(
        !cause_of(&error)?.is_empty(),
        "the operating system said why"
    );
    assert!(!target.exists(), "a refused write creates nothing");
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
            unchanged_identity(target, PRIVATE_FILE_MODE, original)
        })
        .refused_because("a target that changed identity is not overwritten")?;
    assert_eq!(error.first_id(), Some(ErrorId::TargetChangedConcurrently));
    assert_eq!(fs::read_to_string(&target).required()?, "second\n");
    assert!(!dir.path().join(".project.yaml.tmp").exists());
    Ok(())
}

#[test]
fn a_temporary_file_is_named_after_the_target_and_sits_beside_it() -> Checked {
    // 名前が決まっているからこそ、中断した実行の残骸を次回起動時に見つけられる。
    let dir = temp_dir()?;
    let temp = temp_path_for(&dir.path().join("project.yaml"))
        .required_because("a target with a parent and a name")?;
    assert_eq!(temp, dir.path().join(".project.yaml.tmp"));
    Ok(())
}

#[test]
fn a_target_without_a_parent_or_a_file_name_has_nowhere_to_hold_a_temporary_file() -> Checked {
    let dir = temp_dir()?;
    // rootには親directoryが無く、`..`で終わるpathには置き換える名前が無い。
    for (target, expected) in [
        (Path::new("/").to_path_buf(), "cause-no-parent-directory"),
        (dir.path().join(".."), "cause-no-file-name"),
    ] {
        let error = temp_path_for(&target)
            .refused_because("a path that names no file in a directory is refused")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::AtomicWriteFailed),
            "{target:?} produced the wrong error"
        );
        assert_eq!(
            reason_of(&error)?,
            expected,
            "{target:?} named the wrong cause"
        );
    }
    Ok(())
}

#[test]
fn a_target_that_is_not_there_is_never_created_by_a_replacement() -> Checked {
    // 置き換えは既存fileの入れ替えである。相手が居ない場合に新規作成へ落とさない。
    let dir = temp_dir()?;
    let target = dir.path().join("project.yaml");

    let error = atomic_replace(&target, "version: 1\n", PRIVATE_FILE_MODE)
        .refused_because("a file that is not there cannot be replaced")?;
    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert_eq!(
        cause_of(&error)?,
        no_such_file(),
        "the diagnostic carries what the operating system said"
    );
    assert!(!target.exists(), "a refused replacement creates nothing");
    assert!(!dir.path().join(".project.yaml.tmp").exists());
    Ok(())
}

#[test]
fn a_rename_into_place_refuses_a_symlink_on_either_side() -> Checked {
    let dir = temp_dir()?;
    let real = dir.path().join("real.tar");
    fs::write(&real, b"real").required_because("seed the file behind the link")?;

    let linked_source = dir.path().join("linked.tar");
    std::os::unix::fs::symlink(&real, &linked_source).required_because("symlink")?;
    let target = dir.path().join("template.tar");
    let error = atomic_rename_into_place(&linked_source, &target)
        .refused_because("a symlinked source is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(!target.exists(), "nothing is published through a link");

    let source = dir.path().join("built.tar");
    fs::write(&source, b"built").required_because("seed the source")?;
    let linked_target = dir.path().join("published.tar");
    std::os::unix::fs::symlink(&real, &linked_target).required_because("symlink")?;
    let error = atomic_rename_into_place(&source, &linked_target)
        .refused_because("a symlinked target is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert_eq!(
        fs::read_to_string(&real).required()?,
        "real",
        "the file behind the link keeps its content"
    );
    assert_eq!(
        fs::read_to_string(&source).required()?,
        "built",
        "the refused source is left where it was"
    );
    Ok(())
}

#[test]
fn a_rename_into_place_refuses_a_source_it_cannot_read_or_that_is_not_a_regular_file() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("template.tar");

    let missing = dir.path().join("missing.tar");
    let error = atomic_rename_into_place(&missing, &target)
        .refused_because("a source that is not there is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    assert_eq!(cause_of(&error)?, no_such_file());

    let directory = dir.path().join("a-directory");
    fs::create_dir(&directory).required_because("create")?;
    fs::write(directory.join("kept"), b"x").required_because("write inside")?;
    let error = atomic_rename_into_place(&directory, &target)
        .refused_because("a directory is not the artifact that was built")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    assert_eq!(
        fs::read_to_string(directory.join("kept")).required()?,
        "x",
        "the refused directory is left alone"
    );
    assert!(!target.exists());
    Ok(())
}

#[test]
fn a_source_that_cannot_take_the_targets_place_stays_where_it_is() -> Checked {
    let dir = temp_dir()?;
    let source = dir.path().join("template.tar");
    fs::write(&source, b"built").required_because("seed the source")?;
    // 既存directoryの上へはrenameできない。失敗しても成果物を捨てない。
    let target = dir.path().join("occupied");
    fs::create_dir(&target).required_because("create")?;
    fs::write(target.join("kept"), b"x").required_because("write inside")?;

    let error = atomic_rename_into_place(&source, &target)
        .refused_because("a target that cannot be replaced is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert!(
        !cause_of(&error)?.is_empty(),
        "the operating system said why"
    );
    assert_eq!(
        fs::read_to_string(&source).required()?,
        "built",
        "the verified artifact survives a failed rename"
    );
    assert_eq!(fs::read_to_string(target.join("kept")).required()?, "x");
    Ok(())
}

#[test]
fn a_target_that_is_still_the_file_that_was_checked_may_be_replaced() -> Checked {
    let dir = temp_dir()?;
    let target = dir.path().join("project.yaml");
    atomic_create(&target, "first\n", PRIVATE_FILE_MODE).required_because("create")?;
    let identity = FileIdentity::of_path_without_following(&target).required()?;

    unchanged_identity(&target, PRIVATE_FILE_MODE, identity)
        .required_because("the file that was checked is the file that is there")?;
    Ok(())
}
