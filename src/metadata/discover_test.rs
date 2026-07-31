use super::*;
use crate::metadata::render;
use crate::testing::metadata::{attached, write_project};

#[test]
fn discovery_returns_every_project_in_canonical_order() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    for (owner, repository) in [("zeta", "repo"), ("alpha", "repo"), ("alpha", "another")] {
        write_project(
            dir.path(),
            owner,
            repository,
            &render(&attached(owner, repository)),
        );
    }

    let found = discover(&base).expect("every project parses");
    let ids: Vec<String> = found
        .iter()
        .map(|project| project.metadata.canonical_id().to_string())
        .collect();
    assert_eq!(ids, vec!["alpha/another", "alpha/repo", "zeta/repo"]);
}

#[test]
fn discovery_looks_at_the_documented_shape_only() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    write_project(
        dir.path(),
        "example-org",
        "example-repo",
        &render(&attached("example-org", "example-repo")),
    );
    // 深すぎるpath、`.project`で終わらないdirectory、metadataのないdirectory。
    fs::create_dir_all(dir.path().join("owner/deeper/nested.project/.sbxm")).unwrap();
    fs::write(
        dir.path()
            .join("owner/deeper/nested.project/.sbxm/project.yaml"),
        render(&attached("owner", "nested")),
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("owner/plain-directory")).unwrap();

    let found = discover(&base).expect("only the documented shape is read");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].metadata.canonical_id().to_string(),
        "example-org/example-repo"
    );
}

#[test]
fn a_symlinked_project_directory_is_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    let real = write_project(
        dir.path(),
        "example-org",
        "example-repo",
        &render(&attached("example-org", "example-repo")),
    );
    fs::create_dir_all(dir.path().join("linked-org")).unwrap();
    std::os::unix::fs::symlink(&real, dir.path().join("linked-org/example-repo.project")).unwrap();

    let found = discover(&base).expect("the symlink is skipped rather than followed");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].paths.root(), real);
}

#[test]
fn one_broken_project_stops_the_whole_listing() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    write_project(
        dir.path(),
        "example-org",
        "example-repo",
        &render(&attached("example-org", "example-repo")),
    );
    write_project(dir.path(), "broken-org", "broken-repo", "version: 2\n");

    let error = discover(&base).expect_err("a partial listing is never returned");
    assert!(
        error.contains_id(ErrorId::MetadataUnknownVersion),
        "{error:?}"
    );
}

#[test]
fn a_project_stored_outside_its_derived_path_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    // metadataはexample-orgの案件だが、別のowner directoryへ置かれている。
    write_project(
        dir.path(),
        "other-org",
        "example-repo",
        &render(&attached("example-org", "example-repo")),
    );

    let error = discover(&base).expect_err("the derived path is the only place a project lives");
    assert!(
        error.contains_id(ErrorId::MetadataPathMismatch),
        "{error:?}"
    );
}

fn registered(canonical_id: &str, sandbox_name: &str, root: &str) -> Registered {
    Registered {
        canonical_id: canonical_id.to_string(),
        sandbox_name: sandbox_name.to_string(),
        root: root.to_string(),
    }
}

#[test]
fn distinct_projects_are_not_a_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let base = AbsoluteBasePath::new(dir.path()).unwrap();
    let projects: Vec<DiscoveredProject> = [("example-org", "example-repo"), ("other", "repo")]
        .into_iter()
        .map(|(owner, repository)| {
            let metadata = attached(owner, repository);
            DiscoveredProject {
                paths: ProjectPaths::derive(&base, metadata.canonical_id()),
                metadata,
            }
        })
        .collect();
    assert!(conflicts(&projects).is_empty());
}

#[test]
fn two_directories_claiming_the_same_project_are_listed_as_a_conflict() {
    let diagnostics = conflicts_of(&[
        registered("example-org/example-repo", "sbxm-a", "/base/a.project"),
        registered("example-org/example-repo", "sbxm-a", "/base/b.project"),
    ]);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, ErrorId::MetadataDuplicateProject);
}

#[test]
fn two_projects_that_derive_the_same_sandbox_name_are_listed_as_a_conflict() {
    // hash prefixの衝突は実際には起こせないため、導出結果の対応だけを検査する。
    let diagnostics = conflicts_of(&[
        registered("example-org/example-repo", "sbxm-shared", "/base/a.project"),
        registered("other-org/other-repo", "sbxm-shared", "/base/b.project"),
    ]);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, ErrorId::SandboxNameCollision);
}
