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
