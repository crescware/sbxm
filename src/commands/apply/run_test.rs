use super::super::fake::*;
use super::*;
use crate::hash::sha256_hex;
use crate::metadata::RebuildIntent;
use crate::paths::{LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope};
use crate::testing::value::DIGEST;
use std::os::unix::fs::PermissionsExt;

/// 既存testは宣言fileの配置を確かめる。
const FILES_ONLY: Scope = Scope {
    files: true,
    worktrees: None,
};

#[test]
fn asking_for_worktrees_leaves_the_declared_files_alone() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();

    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    let paths = write_metadata(&config, None);
    let host = FakeSbx::listing(&listing(&workspace_root, "running")).holding_repository();

    let output = run(&config, &project(), WORKTREES_ONLY, &host, &workspace_root)
        .expect("worktrees are applied on their own");

    assert_eq!(output.worktrees, Some(3));
    assert!(output.files.is_empty());
    // 宣言fileの配置は既存のfileを上書きする。名指していない対象へは触れない。
    assert!(
        !host.ran("cp --follow-link"),
        "the declared files were not asked for"
    );

    let stored = metadata::load(&paths).unwrap().expect("present");
    assert_eq!(stored.provisioning.requested_worktrees, 3);
}

#[test]
fn a_number_below_what_the_project_has_is_refused() {
    let (_home, config, workspace_root) = setup(Vec::new());
    let paths = write_metadata(&config, None);
    let mut metadata = metadata::load(&paths).unwrap().expect("present");
    metadata.provisioning.requested_worktrees = 3;
    metadata::update(&paths, &metadata).unwrap();
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    let scope = Scope {
        files: false,
        worktrees: Some(2),
    };
    let error = run(&config, &project(), scope, &host, &workspace_root)
        .expect_err("removing a worktree deletes what is checked out in it");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesNotReducible));

    let stored = metadata::load(&paths).unwrap().expect("present");
    assert_eq!(
        stored.provisioning.requested_worktrees, 3,
        "a refused run leaves the target where it was"
    );
}

#[test]
fn a_running_project_gets_the_declared_files_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();
    let _ = sha256_hex(b"declared = true\n");

    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    write_metadata(&config, None);
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    let output = run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect("sync");
    assert_eq!(output.project, "Example-Org/Example-Repo");
    assert_eq!(output.files.len(), 1);
    assert!(host.ran("cp --follow-link"));
}

#[test]
fn the_project_lock_is_held_while_the_files_are_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();

    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    let paths = write_metadata(&config, None);
    let host =
        FakeSbx::listing(&listing(&workspace_root, "running")).watching_lock(paths.lock_file());

    run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect("sync");

    assert_eq!(
        *host.lock_was_free.borrow(),
        Some(false),
        "another run must not reach the same sandbox while files are being replaced"
    );
    assert_eq!(
        std::fs::metadata(paths.lock_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        PRIVATE_FILE_MODE
    );

    // lockはworkflow終了後に解放され、lock file自体は残る。
    crate::paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect("the lock is released when the workflow ends");
}

#[test]
fn a_project_that_is_not_managed_gets_no_lock_file() {
    let (_home, config, workspace_root) = setup(Vec::new());
    let host = FakeSbx::listing("[]");

    run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect_err("nothing to place");

    let paths = ProjectPaths::derive(&config.base_path, &canonical());
    assert!(
        !paths.lock_file().exists(),
        "an unmanaged project is not given a lock file"
    );
}

#[test]
fn nothing_else_in_the_project_is_touched() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();
    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    let paths = write_metadata(&config, None);
    let before = std::fs::read_to_string(paths.metadata_file()).unwrap();
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect("sync");

    for forbidden in [
        "build",
        "image save",
        "template load",
        "worktree add",
        "clone",
    ] {
        assert!(
            !host.ran(forbidden),
            "sync-files must not run {forbidden}: {:?}",
            host.calls()
        );
    }
    assert_eq!(
        std::fs::read_to_string(paths.metadata_file()).unwrap(),
        before,
        "the metadata is read-only for sync-files"
    );
}

#[test]
fn a_stopped_sandbox_is_not_started_and_the_user_is_sent_to_open() {
    let (_home, config, workspace_root) = setup(Vec::new());
    write_metadata(&config, None);
    let host = FakeSbx::listing(&listing(&workspace_root, "stopped"));

    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a stopped sandbox is not started implicitly");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    assert_eq!(
        error.diagnostics()[0]
            .remediation
            .as_ref()
            .map(|message| message.id),
        Some("remediation-sandbox-not-running")
    );
    // 起動commandは`exec <name> -- /bin/true`であるため、一覧以外が走っていないことで見る。
    let beyond_listing: Vec<Vec<String>> = host
        .calls()
        .into_iter()
        .filter(|args| args.first().map(String::as_str) != Some("ls"))
        .collect();
    assert!(
        beyond_listing.is_empty(),
        "a stopped sandbox is not started implicitly: {beyond_listing:?}"
    );
}

#[test]
fn a_project_that_is_not_managed_or_not_built_is_refused() {
    let (_home, config, workspace_root) = setup(Vec::new());
    let host = FakeSbx::listing("[]");
    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("an unregistered project has nowhere to place files");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

    write_metadata(&config, None);
    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a registered project without a sandbox has nowhere to place files");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
}

#[test]
fn a_rebuild_in_progress_places_nothing() {
    let (_home, config, workspace_root) = setup(Vec::new());
    write_metadata(
        &config,
        Some(RebuildIntent {
            target_dockerfile_sha256: "2".repeat(64),
            previous_dockerfile_sha256: DIGEST.into(),
        }),
    );
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a half-switched sandbox is not the target of a placement");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(host.calls().is_empty(), "nothing is asked of the runtime");
}

#[test]
fn a_sandbox_that_belongs_to_another_project_is_refused() {
    let (_home, config, workspace_root) = setup(Vec::new());
    write_metadata(&config, None);
    let name = SandboxName::derive(&canonical());
    let host = FakeSbx::listing(&format!(
        r#"[{{"name":"{name}","state":"running","workspace":"/tmp/elsewhere","template":"other:1"}}]"#
    ));

    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a sandbox that cannot be identified is not written to");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
}
