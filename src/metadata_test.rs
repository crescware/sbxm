use super::*;
use crate::testing::value::DIGEST;
use std::os::unix::fs::PermissionsExt;

fn canonical(value: &str) -> CanonicalProjectId {
    ProjectId::parse(value)
        .expect("valid project id")
        .canonical()
}

const OTHER_DIGEST: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn attached(owner: &str, repository: &str) -> ProjectMetadata {
    ProjectMetadata {
        owner: owner.to_string(),
        repository: repository.to_string(),
        canonical_id: canonical(&format!("{owner}/{repository}")),
        provisioning: Provisioning {
            mode: CreationMode::Attached,
            start_ref: Some("main".to_string()),
            requested_worktrees: 1,
            dockerfile_sha256: DIGEST.to_string(),
        },
        rebuild: None,
    }
}

fn round_trip(metadata: &ProjectMetadata) -> ProjectMetadata {
    parse(&render(metadata), Path::new("/tmp/project.toml")).expect("the rendered form parses")
}

fn write_project(base: &Path, owner: &str, repository: &str, text: &str) -> PathBuf {
    let root = base
        .join(owner.to_ascii_lowercase())
        .join(format!("{}.project", repository.to_ascii_lowercase()));
    let dir = root.join(".sbxm");
    fs::create_dir_all(&dir).expect("create .sbxm");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("mode");
    let path = dir.join("project.toml");
    fs::write(&path, text).expect("write metadata");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode");
    root
}

#[test]
fn metadata_written_before_worktrees_stopped_being_recorded_still_parses() {
    // 記録していた時期のfile。managed worktreeは本数から導けるため読む必要がなく、
    // 残っていても案件の目標構成は変わらない。
    let text = "\
version = 1
owner = \"Example-Org\"
repository = \"Example-Repo\"
canonical_id = \"example-org/example-repo\"

[provisioning]
mode = \"detached\"
start_ref = \"develop\"
requested_worktrees = 2
dockerfile_sha256 = \"1111111111111111111111111111111111111111111111111111111111111111\"

[[worktrees.managed]]
path = \"example-repo.tree-0\"
created_from = \"refs/remotes/origin/develop\"
";
    let parsed = parse(text, Path::new("/tmp/project.toml")).expect("the older form parses");
    assert_eq!(parsed.provisioning.requested_worktrees, 2);
    assert_eq!(parsed.provisioning.start_ref.as_deref(), Some("develop"));
}

#[test]
fn metadata_round_trips_through_the_rendered_form() {
    let metadata = attached("Example-Org", "Example-Repo");
    assert_eq!(round_trip(&metadata), metadata);

    let detached = ProjectMetadata {
        provisioning: Provisioning {
            mode: CreationMode::Detached,
            start_ref: Some("develop".to_string()),
            requested_worktrees: 3,
            dockerfile_sha256: DIGEST.to_string(),
        },
        rebuild: Some(RebuildIntent {
            target_dockerfile_sha256: OTHER_DIGEST.to_string(),
            previous_dockerfile_sha256: DIGEST.to_string(),
        }),
        ..attached("example-org", "example-repo")
    };
    assert_eq!(round_trip(&detached), detached);
}

#[test]
fn an_attached_project_may_wait_for_the_remote_default_branch() {
    let mut metadata = attached("example-org", "example-repo");
    metadata.provisioning.start_ref = None;
    assert_eq!(round_trip(&metadata), metadata);

    // detached modeは起点branchの明示を必須とする。
    let text = render(&metadata).replace("attached", "detached");
    let error = parse(&text, Path::new("/tmp/project.toml"))
        .expect_err("a detached project needs a start branch");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
}

#[test]
fn an_unknown_version_is_diagnosed_before_other_fields() {
    let text = "version = 2\n";
    let error = parse(text, Path::new("/tmp/project.toml")).expect_err("unknown versions fail");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataUnknownVersion));
}

#[test]
fn required_fields_are_named_when_they_are_missing() {
    let full = render(&attached("example-org", "example-repo"));
    for field in [
        "owner = ",
        "repository = ",
        "canonical_id = ",
        "mode = ",
        "start_ref = ",
        "requested_worktrees = ",
        "dockerfile_sha256 = ",
    ] {
        let text: String = full
            .lines()
            .filter(|line| !line.starts_with(field))
            .map(|line| format!("{line}\n"))
            .collect();
        let error = parse(&text, Path::new("/tmp/project.toml")).expect_err("{field} is required");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::MetadataMissingField),
            "{field} produced the wrong error"
        );
    }
}

#[test]
fn the_canonical_identifier_must_match_the_display_names() {
    let text = render(&attached("Example-Org", "Example-Repo"))
        .replace("example-org/example-repo", "other-org/other-repo");
    let error = parse(&text, Path::new("/tmp/project.toml"))
        .expect_err("the canonical ID must fold from owner and repository");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));

    let text = render(&attached("example-org", "example-repo")).replace(
        "canonical_id = \"example-org",
        "canonical_id = \"Example-Org",
    );
    let error = parse(&text, Path::new("/tmp/project.toml"))
        .expect_err("the canonical ID is always folded");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
}

#[test]
fn values_outside_the_documented_range_are_refused() {
    let base = render(&attached("example-org", "example-repo"));
    for (from, to) in [
        ("requested_worktrees = 1", "requested_worktrees = 0"),
        ("requested_worktrees = 1", "requested_worktrees = 33"),
        ("mode = \"attached\"", "mode = \"half-attached\""),
        ("dockerfile_sha256 = \"1111", "dockerfile_sha256 = \"NOTHEX"),
    ] {
        let text = base.replace(from, to);
        assert_ne!(text, base, "the replacement {from} did not apply");
        let error = parse(&text, Path::new("/tmp/project.toml")).expect_err("{to} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::MetadataInvalidValue),
            "{to} produced the wrong error"
        );
    }
}

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
        .map(|project| project.metadata.canonical_id.to_string())
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
            .join("owner/deeper/nested.project/.sbxm/project.toml"),
        render(&attached("owner", "nested")),
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("owner/plain-directory")).unwrap();

    let found = discover(&base).expect("only the documented shape is read");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].metadata.canonical_id.to_string(),
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
    write_project(dir.path(), "broken-org", "broken-repo", "version = 2\n");

    let error = discover(&base).expect_err("a partial listing is never returned");
    assert!(error.contains(ErrorId::MetadataUnknownVersion), "{error:?}");
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
    assert!(error.contains(ErrorId::MetadataPathMismatch), "{error:?}");
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
                paths: ProjectPaths::derive(&base, &metadata.canonical_id),
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
