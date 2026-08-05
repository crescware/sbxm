use crate::design::Fact;
use crate::diagnostics::ErrorId;
use crate::paths::ProjectPaths;
use std::fs;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::metadata::render;
use crate::paths::ProjectParent;
use crate::testing::metadata::{attached, canonical};
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_worktree_count_is_converted_to_a_zero_based_index() {
    assert_eq!(last_worktree_index(MIN_WORKTREES), 0);
    assert_eq!(last_worktree_index(MIN_WORKTREES + 1), 1);
    assert_eq!(
        last_worktree_index(MAX_WORKTREES),
        MAX_WORKTREE_INDEX,
        "the optimistic ceiling is the configured maximum count, not a second literal"
    );
}

/// 診断が持つ、外部が述べた原因。
fn cause(error: &crate::diagnostics::Error) -> Checked<String> {
    error
        .diagnostics()
        .iter()
        .flat_map(|diagnostic| &diagnostic.facts)
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-cause-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the diagnostic carries the reported cause")
}

#[test]
fn metadata_is_written_privately_and_replaced_in_place() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let base = ProjectParent::at(dir.path()).required()?;
    let metadata = attached("example-org", "example-repo")?;
    let project = ProjectPaths::derive(&base, metadata.canonical_id());
    fs::create_dir_all(project.sbxm_dir()).required()?;

    create(&project, &metadata).required_because("create")?;
    let mode = fs::metadata(project.metadata_file())
        .required()?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(load(&project).required()?.as_ref(), Some(&metadata));

    let mut resolved = metadata.clone();
    resolved.provisioning.start_ref = Some("develop".to_string());
    update(&project, &resolved).required_because("update")?;
    assert_eq!(load(&project).required()?, Some(resolved));
    Ok(())
}

#[test]
fn a_missing_metadata_file_is_not_an_error_but_a_symlinked_one_is() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let base = ProjectParent::at(dir.path()).required()?;
    let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo")?);
    assert_eq!(load(&project).required()?, None);

    fs::create_dir_all(project.sbxm_dir()).required()?;
    let real = dir.path().join("elsewhere.yaml");
    fs::write(&real, render(&attached("example-org", "example-repo")?)?).required()?;
    std::os::unix::fs::symlink(&real, project.metadata_file()).required()?;

    let error = load(&project).refused_because("a symlinked metadata file is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
    Ok(())
}

#[test]
fn a_metadata_directory_that_is_not_a_directory_is_reported_with_the_cause_the_os_gave() -> Checked
{
    let dir = tempfile::tempdir().required()?;
    let base = ProjectParent::at(dir.path()).required()?;
    let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo")?);
    // `.sbxm`がfileであるproject root。metadataは不在ではなく、観測できない。
    fs::create_dir_all(project.root()).required()?;
    fs::write(project.sbxm_dir(), b"").required()?;

    let error = load(&project).refused_because("the metadata path cannot be observed")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
    // 述べるのはOSが返した原文であり、sbxmの言い換えでも「不在」でもない。
    let reported = fs::symlink_metadata(project.metadata_file())
        .err()
        .required_because("the same observation fails in the test")?;
    assert_eq!(cause(&error)?, reported.to_string());
    Ok(())
}

#[test]
fn metadata_that_is_not_text_is_refused_rather_than_read_as_absent() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let base = ProjectParent::at(dir.path()).required()?;
    let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo")?);
    fs::create_dir_all(project.sbxm_dir()).required()?;
    // 有効なUTF-8にならないbyte列。fileは在るが原文としては読めない。
    let original: &[u8] = b"version: 1\nrepository: \xff\n";
    fs::write(project.metadata_file(), original).required()?;

    let error = load(&project).refused_because("bytes that are not text cannot be read")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
    let reported = fs::read_to_string(project.metadata_file())
        .err()
        .required_because("the same read fails in the test")?;
    assert_eq!(cause(&error)?, reported.to_string());
    assert_eq!(
        fs::read(project.metadata_file()).required()?,
        original,
        "metadata that could not be read is not rewritten"
    );
    Ok(())
}

#[test]
fn a_metadata_path_that_is_not_a_regular_file_is_refused_before_it_is_read() -> Checked {
    // 特殊fileを開くと、読み取りがそのまま待ちに入り得る。読む前に型で拒否する。
    let dir = tempfile::tempdir().required()?;
    let base = ProjectParent::at(dir.path()).required()?;
    let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo")?);
    fs::create_dir_all(project.metadata_file()).required()?;

    let error =
        load(&project).refused_because("a metadata path that is not a regular file is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
    let reason = error.diagnostics()[0]
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::Translated { value, .. } => Some(value.id),
            _ => None,
        })
        .required_because("the observed reason is named")?;
    assert_eq!(reason, "cause-not-a-regular-file");
    Ok(())
}
