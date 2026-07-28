use super::*;
use crate::config::{GitIdentity, GlobalConfig};
use crate::i18n::Locale;
use crate::metadata::RebuildIntent;
use crate::paths::AbsoluteBasePath;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

fn setup() -> (tempfile::TempDir, GlobalConfig) {
    let dir = tempfile::tempdir().expect("temporary base path");
    let config = GlobalConfig {
        language: Locale::En,
        base_path: AbsoluteBasePath::new(dir.path()).expect("valid base path"),
        git: GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        },
        files: Vec::new(),
    };
    (dir, config)
}

pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> AddRequest {
    AddRequest {
        project: ProjectId::parse(project).expect("valid project id"),
        worktrees,
        detach: detach.map(|value| value.to_string()),
    }
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path)
        .expect("the path exists")
        .permissions()
        .mode()
        & 0o777
}

pub const COMMIT: &str = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";

/// `sbx ls`だけを答え、Sandbox内のcommandは成功として扱うhost。
#[test]
fn registering_a_project_creates_the_documented_layout() {
    let (_dir, config) = setup();
    let registration =
        register(&config, &request("Example-Org/Example-Repo", None, None)).expect("register");

    let paths = &registration.paths;
    assert!(paths.root().is_dir());
    assert_eq!(mode_of(&paths.sbxm_dir()), PRIVATE_DIR_MODE);
    assert_eq!(mode_of(&paths.cache_dir()), PRIVATE_DIR_MODE);
    assert_eq!(mode_of(&paths.metadata_file()), PRIVATE_FILE_MODE);
    assert_eq!(mode_of(&paths.dockerfile()), PRIVATE_FILE_MODE);

    // 表示にはGitHub上の表記、突き合わせにはcanonical形式を使う。
    let metadata = &registration.metadata;
    assert_eq!(metadata.display_id(), "Example-Org/Example-Repo");
    assert_eq!(
        metadata.canonical_id.to_string(),
        "example-org/example-repo"
    );
    assert_eq!(
        registration.sandbox.as_str(),
        metadata.sandbox_name().as_str()
    );

    let stored = metadata::load(paths).expect("load").expect("present");
    assert_eq!(&stored, metadata);
    assert_eq!(stored.provisioning.mode, CreationMode::Attached);
    assert_eq!(stored.provisioning.start_ref, None);
    assert_eq!(stored.provisioning.requested_worktrees, 1);
    assert_eq!(
        stored.provisioning.dockerfile_sha256,
        current_dockerfile_hash(&registration.paths).expect("the adopted Dockerfile")
    );
}

#[test]
fn the_bundled_dockerfile_is_written_once_and_never_edited_again() {
    let (_dir, config) = setup();
    let registration =
        register(&config, &request("example-org/example-repo", None, None)).expect("register");
    let dockerfile = registration.paths.dockerfile();
    assert_eq!(
        fs::read_to_string(&dockerfile).unwrap(),
        BUNDLED_DOCKERFILE,
        "a new project starts from the bundled template"
    );
    drop(registration);

    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let registration =
        register(&config, &request("example-org/example-repo", None, None)).expect("register");
    assert_eq!(
        fs::read_to_string(&dockerfile).unwrap(),
        "FROM scratch\n",
        "an edited Dockerfile belongs to the user"
    );
    assert_eq!(
        current_dockerfile_hash(&registration.paths).expect("the adopted Dockerfile"),
        sha256_hex(b"FROM scratch\n"),
        "the current content decides the current hash"
    );
    assert_eq!(
        registration.metadata.provisioning.dockerfile_sha256,
        sha256_hex(BUNDLED_DOCKERFILE.as_bytes()),
        "the applied generation stays as recorded until an image is built"
    );
}

#[test]
fn the_bundled_dockerfile_meets_the_rules_it_ships_under() {
    assert!(
            BUNDLED_DOCKERFILE.contains(
                "docker.io/docker/sandbox-templates:shell-docker@sha256:39cf20eca861ec92747487af6197f6d916f774bdb98245d267dbd8dfd3debb05"
            ),
            "the base image stays pinned by digest"
        );
    for tool in [
        "git",
        "openssh-client",
        "coreutils",
        "ca-certificates",
        "curl",
        "wget",
        "gh",
        "jq",
    ] {
        assert!(
            BUNDLED_DOCKERFILE.contains(tool),
            "the fixed tool set installs {tool}"
        );
    }
    assert!(BUNDLED_DOCKERFILE.contains("WORKDIR /home/agent/work"));
    assert!(BUNDLED_DOCKERFILE.contains("-o agent -g agent"));
    assert!(
        !BUNDLED_DOCKERFILE.contains("GH_TOKEN"),
        "no token, real or sentinel, is written into the image"
    );
    for line in BUNDLED_DOCKERFILE.lines() {
        let instruction = line.trim_start();
        assert!(
            !instruction.starts_with("COPY ") && !instruction.starts_with("ADD "),
            "the build context is empty, so nothing can be copied into the image: {line}"
        );
    }
}

/// 同梱するtoolのversionを決めるARG。
///
/// templateが取りに行く先はこの値だけで決まる。値を消して常に最新を取る形へ
/// 書き換えられていないことを、この並びで確かめる。
const PINNED_TOOLS: [&str; 4] = [
    "GH_VERSION",
    "CLAUDE_CODE_VERSION",
    "CODEX_VERSION",
    "MISE_VERSION",
];

#[test]
fn the_bundled_dockerfile_pins_every_tool_it_installs() {
    for name in PINNED_TOOLS {
        let declared = BUNDLED_DOCKERFILE
            .lines()
            .map(str::trim_start)
            .find_map(|line| line.strip_prefix(&format!("ARG {name}=")))
            .unwrap_or_else(|| panic!("{name} names the version this template installs"));
        assert!(
            !declared.trim().is_empty(),
            "{name} has to name a version rather than leave it open"
        );
        // 宣言した値を実際に取りに行っていなければ、宣言はpinとして働かない。
        assert!(
            BUNDLED_DOCKERFILE.contains(&format!("${{{name}}}")),
            "the version {name} names has to be the one that is fetched"
        );
    }

    // 取得先が動く参照であれば、versionを書いていてもpinにならない。
    assert!(
        !BUNDLED_DOCKERFILE.contains("latest"),
        "a moving reference is not a pin"
    );
}

#[test]
fn the_options_decide_the_target_configuration() {
    let (_dir, config) = setup();
    let cases = [
        ("one/repo", None, None, CreationMode::Attached, None, 1),
        ("two/repo", Some(1), None, CreationMode::Attached, None, 1),
        (
            "three/repo",
            None,
            Some("develop"),
            CreationMode::Detached,
            Some("develop"),
            1,
        ),
        (
            "four/repo",
            Some(1),
            Some("develop"),
            CreationMode::Detached,
            Some("develop"),
            1,
        ),
        (
            "five/repo",
            Some(3),
            Some("develop"),
            CreationMode::Detached,
            Some("develop"),
            3,
        ),
    ];

    for (project, worktrees, detach, mode, start_ref, count) in cases {
        let registration =
            register(&config, &request(project, worktrees, detach)).expect("register");
        let provisioning = &registration.metadata.provisioning;
        assert_eq!(provisioning.mode, mode, "{project}");
        assert_eq!(provisioning.start_ref.as_deref(), start_ref, "{project}");
        assert_eq!(provisioning.requested_worktrees, count, "{project}");
    }

    // 2個以上のmanaged worktreeは起点branchの明示を必須とする。
    let error = register(&config, &request("six/repo", Some(2), None))
        .expect_err("two worktrees need an explicit branch");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));
}

#[test]
fn an_unusable_start_branch_stops_before_anything_is_created() {
    let (dir, config) = setup();
    let error = register(
        &config,
        &request("example-org/example-repo", None, Some("-x")),
    )
    .expect_err("a branch that could be read as an option is refused");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "nothing may be created before the input is accepted"
    );
}

#[test]
fn re_running_add_without_options_continues_from_the_stored_target() {
    let (_dir, config) = setup();
    let first = register(
        &config,
        &request("example-org/example-repo", Some(3), Some("develop")),
    )
    .expect("register");
    let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
    drop(first);

    let again =
        register(&config, &request("example-org/example-repo", None, None)).expect("re-run");
    assert_eq!(again.metadata.provisioning.requested_worktrees, 3);
    assert_eq!(
        again.metadata.provisioning.start_ref.as_deref(),
        Some("develop")
    );
    assert_eq!(
        fs::read_to_string(again.paths.metadata_file()).unwrap(),
        before,
        "a re-run must not rewrite the stored target"
    );
}

#[test]
fn options_that_disagree_with_the_stored_target_stop_the_run() {
    let (_dir, config) = setup();
    let first = register(
        &config,
        &request("example-org/example-repo", Some(3), Some("develop")),
    )
    .expect("register");
    let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
    drop(first);

    // 完全に一致するoptionは受け付ける。
    register(
        &config,
        &request("example-org/example-repo", Some(3), Some("develop")),
    )
    .expect("the same options continue the build");

    for (worktrees, detach) in [
        (Some(2), Some("develop")),
        (Some(3), Some("main")),
        (Some(1), Some("develop")),
        (None, Some("main")),
    ] {
        let error = register(
            &config,
            &request("example-org/example-repo", worktrees, detach),
        )
        .expect_err("a different target configuration is refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::TargetConfigurationMismatch),
            "{worktrees:?} {detach:?} produced the wrong error"
        );
    }

    // 組み合わせとして成立しないoptionは、保存値と比べる前に拒否する。
    let error = register(&config, &request("example-org/example-repo", Some(3), None))
        .expect_err("two worktrees still need an explicit branch");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));
    assert_eq!(
        fs::read_to_string(
            ProjectPaths::derive(
                &config.base_path,
                &ProjectId::parse("example-org/example-repo")
                    .unwrap()
                    .canonical()
            )
            .metadata_file()
        )
        .unwrap(),
        before
    );
}

#[test]
fn a_rebuild_in_progress_sends_the_user_to_rebuild() {
    let (_dir, config) = setup();
    let registration =
        register(&config, &request("example-org/example-repo", None, None)).expect("register");
    let paths = registration.paths.clone();
    let mut metadata = registration.metadata.clone();
    drop(registration);

    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"target"),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &metadata).expect("record the intent");

    let error = register(&config, &request("example-org/example-repo", None, None))
        .expect_err("add does not continue through a rebuild");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic.remediation.as_ref().map(|message| message.id),
        Some("remediation-run-rebuild")
    );
}

#[test]
fn the_project_lock_is_held_for_the_whole_workflow() {
    let (_dir, config) = setup();
    let registration =
        register(&config, &request("example-org/example-repo", None, None)).expect("register");
    let lock_path = registration.paths.lock_file();
    assert_eq!(mode_of(&lock_path), PRIVATE_FILE_MODE);

    let error = paths::acquire_exclusive_lock(
        &lock_path,
        Duration::from_millis(100),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect_err("a second run waits for the first");
    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

    drop(registration);
    paths::acquire_exclusive_lock(
        &lock_path,
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect("the lock is released when the workflow ends");
}

#[test]
fn a_broken_project_anywhere_under_the_base_path_stops_registration() {
    let (dir, config) = setup();
    let broken = dir.path().join("broken-org").join("broken-repo.project");
    fs::create_dir_all(broken.join(".sbxm")).unwrap();
    fs::write(broken.join(".sbxm").join("project.toml"), "version = 2\n").unwrap();

    let error = register(&config, &request("example-org/example-repo", None, None))
        .expect_err("a listing that cannot be trusted stops the run");
    assert!(error.contains(ErrorId::MetadataUnknownVersion), "{error:?}");
    assert!(
        !dir.path().join("example-org").exists(),
        "nothing may be created while the listing is broken"
    );
}

#[test]
fn an_existing_non_directory_in_the_way_is_refused() {
    let (dir, config) = setup();
    fs::write(dir.path().join("example-org"), b"not a directory").unwrap();

    let error = register(&config, &request("example-org/example-repo", None, None))
        .expect_err("an owner path that is a file is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    assert_eq!(
        fs::read_to_string(dir.path().join("example-org")).unwrap(),
        "not a directory"
    );
}
