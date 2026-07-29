use super::*;
use crate::metadata::render;
use crate::paths::AbsoluteBasePath;
use crate::testing::metadata::{attached, canonical};
use std::os::unix::fs::PermissionsExt;

#[test]
fn metadata_is_written_privately_and_replaced_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    let metadata = attached("example-org", "example-repo");
    let project = ProjectPaths::derive(&base, &metadata.canonical_id);
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
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    let project = ProjectPaths::derive(&base, &canonical("example-org/example-repo"));
    assert_eq!(load(&project).unwrap(), None);

    fs::create_dir_all(project.sbxm_dir()).unwrap();
    let real = dir.path().join("elsewhere.toml");
    fs::write(&real, render(&attached("example-org", "example-repo"))).unwrap();
    std::os::unix::fs::symlink(&real, project.metadata_file()).unwrap();

    let error = load(&project).expect_err("a symlinked metadata file is refused");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnreadable));
}
