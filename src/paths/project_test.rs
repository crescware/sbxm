use super::*;
use crate::testing::fs::temp_dir;

fn base(path: &Path) -> AbsoluteBasePath {
    AbsoluteBasePath::new(path).expect("valid base path")
}

fn project_id(value: &str) -> CanonicalProjectId {
    crate::project::ProjectId::parse(value)
        .expect("valid project id")
        .canonical()
}

#[test]
fn base_path_must_be_absolute() {
    let error = AbsoluteBasePath::new(Path::new("relative/path"))
        .expect_err("relative base paths are rejected");
    assert_eq!(error.first_id(), Some(ErrorId::BasePathNotAbsolute));
}

#[test]
fn base_path_accepts_a_directory_that_does_not_exist_yet() {
    let dir = temp_dir();
    let target = dir.path().join("Projects").join("nested");
    let base = AbsoluteBasePath::new(&target).expect("creatable base paths are accepted");
    assert_eq!(base.as_path(), lexically_standardize(&target));
}

#[test]
fn base_path_rejects_a_symlink_that_escapes_the_declared_root() {
    let dir = temp_dir();
    let real = dir.path().join("real");
    fs::create_dir(&real).expect("create real directory");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("create symlink");

    let error =
        AbsoluteBasePath::new(&link).expect_err("a base path that resolves elsewhere is refused");
    assert_eq!(error.first_id(), Some(ErrorId::BasePathEscapesRoot));
}

#[test]
fn base_path_rejects_an_existing_regular_file() {
    let dir = temp_dir();
    let file = dir.path().join("not-a-directory");
    fs::write(&file, b"x").expect("write file");
    let error = AbsoluteBasePath::new(&file).expect_err("files are not base paths");
    assert_eq!(error.first_id(), Some(ErrorId::BasePathNotDirectory));
}

#[test]
fn project_paths_follow_the_documented_layout() {
    let base = base(Path::new("/Users/example/Projects"));
    let paths = ProjectPaths::derive(&base, &project_id("Example-Org/Example-Repo"));

    // owner名のdirectoryは作らない。project rootは親directoryの直下に並ぶ。
    assert_eq!(
        paths.root(),
        Path::new("/Users/example/Projects/example-repo.project")
    );
    let root = paths.root();
    assert_eq!(paths.host_clone(), root.join("example-repo"));
    assert_eq!(paths.metadata_file(), root.join(".sbxm/project.yaml"));
    assert_eq!(paths.lock_file(), root.join(".sbxm/project.lock"));
    assert_eq!(paths.dockerfile(), root.join(".sbxm/Dockerfile"));
    assert_eq!(paths.cache_dir(), root.join(".sbxm/.cache"));
    assert_eq!(
        paths.template_archive("0123456789ab"),
        root.join(".sbxm/.cache/template-0123456789ab.tar")
    );
    assert_eq!(
        paths.template_archive_temp("0123456789ab"),
        root.join(".sbxm/.cache/template-0123456789ab.tar.tmp")
    );
}

#[test]
fn project_paths_are_lowercase_so_one_project_cannot_take_two_directories() {
    let base = base(Path::new("/Users/example/Projects"));
    assert_eq!(
        ProjectPaths::derive(&base, &project_id("Example-Org/Example-Repo")),
        ProjectPaths::derive(&base, &project_id("example-org/example-repo"))
    );
}

#[test]
fn the_same_repository_name_under_two_owners_wants_the_same_directory() {
    // owner名を足して衝突を避けないため、この2件は同じpathを要求する。衝突の扱いは
    // 登録側が決める。
    let base = base(Path::new("/Users/example/Projects"));
    assert_eq!(
        ProjectPaths::derive(&base, &project_id("example-org/alpha")).root(),
        ProjectPaths::derive(&base, &project_id("other-org/alpha")).root()
    );
}
