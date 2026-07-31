use super::*;
use crate::metadata::render;
use crate::paths::ProjectParent;
use crate::testing::metadata::{attached, canonical};
use std::os::unix::fs::PermissionsExt;

#[test]
fn metadata_is_written_privately_and_replaced_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let base = ProjectParent::at(dir.path()).unwrap();
    let metadata = attached("example-org", "example-repo");
    let project = ProjectPaths::derive(&base, metadata.canonical_id());
    fs::create_dir_all(project.sbxm_dir()).unwrap();

    create(&project, &metadata).expect("create");
    let mode = fs::metadata(project.metadata_file())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(load(&project).unwrap().as_ref(), Some(&metadata));

    let mut resolved = metadata.clone();
    resolved.provisioning.start_ref = Some("develop".to_string());
    update(&project, &resolved).expect("update");
    assert_eq!(load(&project).unwrap(), Some(resolved));
}

#[test]
fn a_missing_metadata_file_is_not_an_error_but_a_symlinked_one_is() {
    let dir = tempfile::tempdir().unwrap();
    let base = ProjectParent::at(dir.path()).unwrap();
    let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo"));
    assert_eq!(load(&project).unwrap(), None);

    fs::create_dir_all(project.sbxm_dir()).unwrap();
    let real = dir.path().join("elsewhere.yaml");
    fs::write(&real, render(&attached("example-org", "example-repo"))).unwrap();
    std::os::unix::fs::symlink(&real, project.metadata_file()).unwrap();

    let error = load(&project).expect_err("a symlinked metadata file is refused");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
}

#[test]
fn a_metadata_path_that_is_not_a_regular_file_is_refused_before_it_is_read() {
    // 特殊fileを開くと、読み取りがそのまま待ちに入り得る。読む前に型で拒否する。
    let dir = tempfile::tempdir().unwrap();
    let base = ProjectParent::at(dir.path()).unwrap();
    let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo"));
    fs::create_dir_all(project.metadata_file()).unwrap();

    let error = load(&project).expect_err("a metadata path that is not a regular file is refused");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
    let detail = error.diagnostics()[0]
        .description
        .args
        .iter()
        .find(|(key, _)| *key == "detail")
        .map(|(_, value)| value.as_str())
        .expect("the observed reason is named");
    assert_eq!(detail, "the metadata path is not a regular file");
}
