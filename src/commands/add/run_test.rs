use super::*;
use crate::config::ConfigLocation;
use crate::metadata::RebuildIntent;
use crate::paths::{LOCK_TIMEOUT, ProjectParent};
use crate::support::generation::current_dockerfile_hash;
use crate::testing::add_request::{from, request};
use crate::testing::project::{https_repository, ssh_repository};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

/// 親directoryとglobal state directoryを持つtest環境。
struct Setup {
    dir: tempfile::TempDir,
    _home: tempfile::TempDir,
    location: ConfigLocation,
    parent: ProjectParent,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().expect("temporary parent directory");
    let home = tempfile::tempdir().expect("temporary home");
    Setup {
        location: ConfigLocation::from_home(home.path().to_path_buf()),
        parent: ProjectParent::at(dir.path()).expect("valid parent directory"),
        dir,
        _home: home,
    }
}

/// hostが宣言しているものとして扱うGit identity。
fn identity() -> crate::metadata::GitIdentity {
    crate::testing::metadata::git_identity()
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path)
        .expect("the path exists")
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn registering_a_project_creates_the_documented_layout() {
    let setup = setup();
    let registration = register(
        &setup.location,
        &setup.parent,
        &request("Example-Org/Example-Repo", None, None),
        &identity(),
    )
    .expect("register");

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
        metadata.canonical_id().to_string(),
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
    let setup = setup();
    let registration = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("register");
    let dockerfile = registration.paths.dockerfile();
    assert_eq!(
        fs::read_to_string(&dockerfile).unwrap(),
        BUNDLED_DOCKERFILE,
        "a new project starts from the bundled template"
    );
    drop(registration);

    fs::write(&dockerfile, "FROM scratch\n").unwrap();
    let registration = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("register");
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
    let setup = setup();
    let cases = [
        ("one/alpha", None, None, CreationMode::Attached, None, 1),
        ("two/bravo", Some(1), None, CreationMode::Attached, None, 1),
        (
            "three/charlie",
            None,
            Some("develop"),
            CreationMode::Detached,
            Some("develop"),
            1,
        ),
        (
            "four/delta",
            Some(1),
            Some("develop"),
            CreationMode::Detached,
            Some("develop"),
            1,
        ),
        (
            "five/echo",
            Some(3),
            Some("develop"),
            CreationMode::Detached,
            Some("develop"),
            3,
        ),
    ];

    for (project, worktrees, detach, mode, start_ref, count) in cases {
        let registration = register(
            &setup.location,
            &setup.parent,
            &request(project, worktrees, detach),
            &identity(),
        )
        .expect("register");
        let provisioning = &registration.metadata.provisioning;
        assert_eq!(provisioning.mode, mode, "{project}");
        assert_eq!(provisioning.start_ref.as_deref(), start_ref, "{project}");
        assert_eq!(provisioning.requested_worktrees, count, "{project}");
    }

    // 2個以上のmanaged worktreeは起点branchの明示を必須とする。
    let error = register(
        &setup.location,
        &setup.parent,
        &request("six/foxtrot", Some(2), None),
        &identity(),
    )
    .expect_err("two worktrees need an explicit branch");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));
}

#[test]
fn an_unusable_start_branch_stops_before_anything_is_created() {
    let setup = setup();
    let error = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, Some("-x")),
        &identity(),
    )
    .expect_err("a branch that could be read as an option is refused");
    assert_eq!(error.first_id(), Some(ErrorId::InvalidBranchName));
    assert_eq!(
        fs::read_dir(setup.dir.path()).unwrap().count(),
        0,
        "nothing may be created before the input is accepted"
    );
}

#[test]
fn re_running_add_without_options_continues_from_the_stored_target() {
    let setup = setup();
    let first = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", Some(3), Some("develop")),
        &identity(),
    )
    .expect("register");
    let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
    drop(first);

    let again = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("re-run");
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
    let setup = setup();
    let first = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", Some(3), Some("develop")),
        &identity(),
    )
    .expect("register");
    let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
    drop(first);

    // 完全に一致するoptionは受け付ける。
    register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", Some(3), Some("develop")),
        &identity(),
    )
    .expect("the same options continue the build");

    for (worktrees, detach) in [
        (Some(2), Some("develop")),
        (Some(3), Some("main")),
        (Some(1), Some("develop")),
        (None, Some("main")),
    ] {
        let error = register(
            &setup.location,
            &setup.parent,
            &request("example-org/example-repo", worktrees, detach),
            &identity(),
        )
        .expect_err("a different target configuration is refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::TargetConfigurationMismatch),
            "{worktrees:?} {detach:?} produced the wrong error"
        );
    }

    // 組み合わせとして成立しないoptionは、保存値と比べる前に拒否する。
    let error = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", Some(3), None),
        &identity(),
    )
    .expect_err("two worktrees still need an explicit branch");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesRequireDetach));
    assert_eq!(
        fs::read_to_string(
            ProjectPaths::derive(
                &setup.parent,
                ssh_repository("example-org/example-repo").canonical_id()
            )
            .metadata_file()
        )
        .unwrap(),
        before
    );
}

#[test]
fn a_rebuild_in_progress_sends_the_user_to_rebuild() {
    let setup = setup();
    let registration = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("register");
    let paths = registration.paths.clone();
    let mut metadata = registration.metadata.clone();
    drop(registration);

    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"target"),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &metadata).expect("record the intent");

    let error = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect_err("add does not continue through a rebuild");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-run-rebuild")
    );
}

#[test]
fn the_project_lock_is_held_for_the_whole_workflow() {
    let setup = setup();
    let registration = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("register");
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
fn a_registry_that_cannot_be_trusted_stops_registration() {
    let setup = setup();
    std::fs::create_dir_all(setup.location.dir()).unwrap();
    std::fs::set_permissions(setup.location.dir(), fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(setup.location.registry_file(), "version: 2\nprojects: []\n").unwrap();
    std::fs::set_permissions(
        setup.location.registry_file(),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    let error = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect_err("a registry that cannot be trusted stops the run");
    assert!(
        error.contains_id(ErrorId::RegistryUnknownVersion),
        "{error:?}"
    );
    assert!(
        !setup.dir.path().join("example-repo.project").exists(),
        "nothing may be created while the registry is broken"
    );
}

#[test]
fn an_existing_artifact_in_the_way_is_never_adopted() {
    // registry entryのない既存成果物のownershipは`add`では確定できない。名前が一致
    // するだけのpathを取り込まず、path collisionとして拒否する。
    let setup = setup();
    for existing in [
        Existing::File(b"not a directory".to_vec()),
        Existing::Directory,
    ] {
        let root = setup.dir.path().join("example-repo.project");
        existing.create(&root);

        let error = register(
            &setup.location,
            &setup.parent,
            &request("example-org/example-repo", None, None),
            &identity(),
        )
        .expect_err("an unregistered artifact is never adopted");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectPathCollision));
        assert!(
            !root.join(".sbxm").exists(),
            "nothing is created inside an artifact sbxm does not own"
        );
        existing.remove(&root);
    }
}

/// 候補pathを塞ぐ既存成果物。
enum Existing {
    File(Vec<u8>),
    Directory,
}

impl Existing {
    fn create(&self, root: &Path) {
        match self {
            Existing::File(contents) => fs::write(root, contents).unwrap(),
            Existing::Directory => fs::create_dir_all(root).unwrap(),
        }
    }

    fn remove(&self, root: &Path) {
        match self {
            Existing::File(_) => fs::remove_file(root).unwrap(),
            Existing::Directory => fs::remove_dir_all(root).unwrap(),
        }
    }
}

#[test]
fn the_same_repository_name_under_another_owner_is_a_path_collision() {
    let setup = setup();
    register(
        &setup.location,
        &setup.parent,
        &request("example-org/alpha", None, None),
        &identity(),
    )
    .expect("the first one registers");

    let error = register(
        &setup.location,
        &setup.parent,
        &request("other-org/alpha", None, None),
        &identity(),
    )
    .expect_err("the second one wants the same directory");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathCollision));

    // 先に登録した案件の成果物は残り、owner名を足したdirectoryも作られない。
    let stored = metadata::load(&ProjectPaths::derive(
        &setup.parent,
        ssh_repository("example-org/alpha").canonical_id(),
    ))
    .expect("load")
    .expect("present");
    assert_eq!(stored.canonical_id().to_string(), "example-org/alpha");
    let entries: Vec<String> = fs::read_dir(setup.dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["alpha.project".to_string()]);
}

#[test]
fn the_same_repository_over_another_transport_is_not_the_same_registration() {
    let setup = setup();
    let first = register(
        &setup.location,
        &setup.parent,
        &request("Example-Org/Example-Repo", None, None),
        &identity(),
    )
    .expect("register");
    let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
    drop(first);

    let error = register(
        &setup.location,
        &setup.parent,
        &from(https_repository("Example-Org/Example-Repo"), None, None),
        &identity(),
    )
    .expect_err("SSH and HTTPS are not the same configuration");
    assert_eq!(error.first_id(), Some(ErrorId::TargetConfigurationMismatch));

    // errorは登録済みのURLで再実行するcommandを示す。
    let diagnostic = &error.diagnostics()[0];
    let remediation = diagnostic
        .remediation
        .as_ref()
        .expect("the user is told which URL the project was registered with");
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(|command| command.as_str())
            .collect::<Vec<_>>(),
        vec!["sbxm add git@github.com:Example-Org/Example-Repo.git"]
    );

    assert_eq!(
        fs::read_to_string(
            ProjectPaths::derive(
                &setup.parent,
                ssh_repository("Example-Org/Example-Repo").canonical_id()
            )
            .metadata_file()
        )
        .unwrap(),
        before,
        "a refused re-run must not rewrite the stored clone URL"
    );
}

#[test]
fn only_the_display_casing_of_the_clone_url_may_differ_on_a_re_run() {
    let setup = setup();
    let first = register(
        &setup.location,
        &setup.parent,
        &request("Example-Org/Example-Repo", None, None),
        &identity(),
    )
    .expect("register");
    let before = fs::read_to_string(first.paths.metadata_file()).unwrap();
    drop(first);

    let again = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("the same repository continues the build");
    assert_eq!(
        again.metadata.repository.clone_url(),
        "git@github.com:Example-Org/Example-Repo.git"
    );
    drop(again);
    assert_eq!(
        fs::read_to_string(
            ProjectPaths::derive(
                &setup.parent,
                ssh_repository("example-org/example-repo").canonical_id()
            )
            .metadata_file()
        )
        .unwrap(),
        before,
        "the stored display spelling is never rewritten"
    );
}

#[test]
fn the_same_parent_directory_holds_more_than_one_repository() {
    let setup = setup();
    for project in ["example-org/alpha", "other-org/beta"] {
        register(
            &setup.location,
            &setup.parent,
            &request(project, None, None),
            &identity(),
        )
        .expect("register");
    }

    let mut entries: Vec<String> = fs::read_dir(setup.dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        vec!["alpha.project".to_string(), "beta.project".to_string()]
    );
}

#[test]
fn a_re_run_uses_the_stored_root_whatever_directory_it_is_run_from() {
    let setup = setup();
    let first = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("register");
    let stored = first.paths.root().to_path_buf();
    drop(first);

    // 別の親directoryから実行しても、保存済みrootだけを対象にする。
    let elsewhere = tempfile::tempdir().expect("another parent directory");
    let again = register(
        &setup.location,
        &ProjectParent::at(elsewhere.path()).expect("valid parent directory"),
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("the stored registration continues");
    assert_eq!(again.paths.root(), stored);
    drop(again);
    assert_eq!(
        fs::read_dir(elsewhere.path()).unwrap().count(),
        0,
        "a registered project is never re-placed by where the command ran"
    );
}

#[test]
fn the_git_identity_the_host_declares_is_snapshotted_into_the_metadata() {
    let setup = setup();
    let registration = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &identity(),
    )
    .expect("register");
    assert_eq!(registration.metadata.git_identity, identity());
    drop(registration);

    // hostの設定が変わっても、登録済み案件のidentityは変わらない。
    let changed = crate::metadata::GitIdentity {
        user_name: "Someone Else".into(),
        user_email: "else@example.com".into(),
    };
    let again = register(
        &setup.location,
        &setup.parent,
        &request("example-org/example-repo", None, None),
        &changed,
    )
    .expect("the stored registration continues");
    assert_eq!(again.metadata.git_identity, identity());
}

#[test]
fn a_run_interrupted_before_its_artifacts_exist_is_continued_by_the_same_add() {
    let setup = setup();
    let request = request("example-org/example-repo", None, None);
    let first = register(&setup.location, &setup.parent, &request, &identity()).expect("register");
    let root = first.paths.root().to_path_buf();
    drop(first);

    // 中断点: registry entryを記録したあと、project rootを作る前。
    fs::remove_dir_all(&root).expect("undo the project root");
    let again = register(&setup.location, &setup.parent, &request, &identity())
        .expect("the entry is continued");
    assert_eq!(
        again.paths.root(),
        root,
        "the recorded intent decides where the project goes"
    );
    assert!(again.paths.metadata_file().is_file());
    drop(again);

    // 中断点: project rootを作ったあと、metadataを書く前。
    fs::remove_file(root.join(".sbxm").join("project.yaml")).expect("undo the metadata");
    let again = register(&setup.location, &setup.parent, &request, &identity())
        .expect("the metadata is created on the next run");
    assert!(again.paths.metadata_file().is_file());
    drop(again);

    // どの中断点からの再開でも、entryは1件のままである。
    let registry = crate::registry::load(&setup.location).expect("the registry stays valid");
    assert_eq!(registry.entries().len(), 1);
}
