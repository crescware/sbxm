use super::*;
use crate::command::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::error::ExitCode;
use crate::metadata;
use crate::testing::{FakeSbx, Fixture, ScriptedPrompt, clean_host, fixture};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

fn poll() -> Poll {
    Poll {
        interval: Duration::from_millis(1),
        limit: Duration::from_millis(20),
    }
}

fn project_id(value: &str) -> ProjectId {
    ProjectId::parse(value).expect("valid project id")
}

fn path_of(target: &Target) -> Option<&str> {
    match target {
        Target::Path(path) => Some(path.as_str()),
        Target::Described(_) => None,
    }
}

#[test]
fn a_clean_running_project_is_planned_then_removed() {
    let fixture = fixture();
    let project = fixture.register("Example-Org/Example-Repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    std::fs::create_dir_all(project.paths.cache_dir()).unwrap();
    // 削除後の一覧では対象が消えている。
    let host = clean_host(&fixture, &project);
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.config,
        Some(&project_id("Example-Org/Example-Repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    assert_eq!(prepared.plan.project, "Example-Org/Example-Repo");
    assert_eq!(prepared.plan.worktrees.len(), 1);
    assert!(
        prepared
            .plan
            .removes
            .iter()
            .any(|target| path_of(target).is_some_and(|path| path.contains("project.toml")))
    );
    assert!(
        prepared
            .plan
            .keeps
            .iter()
            .any(|target| path_of(target).is_some_and(|path| path.contains("Dockerfile")))
    );
    assert_eq!(
        prepared.plan.re_register,
        "sbxm add Example-Org/Example-Repo --worktrees 1"
    );

    let outcome = execute(&host, &prepared, poll()).expect("destroy");
    assert!(outcome.warnings.is_empty());
    assert!(host.ran(&format!("rm --force {}", project.sandbox)));
    assert!(
        !project.paths.metadata_file().exists(),
        "the project is unmanaged now"
    );
    assert!(!project.paths.cache_dir().exists());
    assert!(!project.paths.lock_file().exists());
    assert!(
        project.paths.dockerfile().exists(),
        "the Dockerfile the user edits is kept"
    );
}

#[test]
fn the_removal_shows_its_progress_and_the_listing_is_read_by_sbxm() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = clean_host(&fixture, &project);
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");
    execute(&host, &prepared, poll()).expect("destroy");

    // 外部toolの進捗は隠さず、SSH Agentを渡さず、lifecycleのtimeoutで実行する。
    let removal = host.spec(&format!("rm --force {}", project.sandbox));
    assert_eq!(removal.output, OutputPolicy::Passthrough);
    assert_eq!(removal.env, EnvPolicy::InheritWithoutSshAgent);
    assert_eq!(removal.timeout, TimeoutClass::SandboxLifecycle);

    // 判定に使う出力はsbxmが読む。
    let listing = host.spec("ls --json");
    assert_eq!(listing.output, OutputPolicy::Capture);
    let inspection = host.spec("worktree list --porcelain -z");
    assert_eq!(inspection.output, OutputPolicy::Capture);
    assert_eq!(inspection.env, EnvPolicy::InheritWithoutSshAgent);
}

#[test]
fn a_stopped_project_is_refused_in_the_normal_mode_and_removed_with_force() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let stopped = format!("[{}]", fixture.entry(&project, "stopped"));

    let host = FakeSbx::listing(&stopped);
    let error = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect_err("a stopped sandbox cannot be inspected");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));

    let host = FakeSbx::listings(&[&stopped, "[]"]);
    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        true,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("force skips the checks");
    assert!(prepared.plan.force);
    assert!(prepared.plan.worktrees.is_empty());

    execute(&host, &prepared, poll()).expect("destroy");
    // `--force`は`sbx`の確認promptを省くためのもので、常に付ける。sbxm側の
    // `--force`はsbxm自身のデータ保護検査を省くことを指す。
    assert!(host.ran(&format!("rm --force {}", project.sandbox)));
    assert!(!project.paths.metadata_file().exists());
}

#[test]
fn unsaved_work_stops_the_normal_mode_before_anything_is_deleted() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let host = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "1 .M N... 100644 100644 100644 abc abc file.txt\0",
    );

    let error = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect_err("work that only exists here is not deleted");
    assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
    assert!(!host.ran("rm "), "nothing is removed");
    assert!(project.paths.metadata_file().exists());
}

#[test]
fn a_project_without_a_sandbox_only_loses_its_management_data() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = FakeSbx::listing("[]");

    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");
    assert_eq!(prepared.plan.state, ProjectState::NotCreated);

    execute(&host, &prepared, poll()).expect("destroy");
    assert!(!host.ran("rm "), "there is no sandbox to remove");
    assert!(!project.paths.metadata_file().exists());
}

#[test]
fn the_re_registration_command_repeats_the_target_configuration() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let mut metadata = project.metadata.clone();
    metadata.provisioning.mode = CreationMode::Detached;
    metadata.provisioning.start_ref = Some("develop".into());
    metadata.provisioning.requested_worktrees = 3;
    metadata::update(&project.paths, &metadata).unwrap();

    assert_eq!(
        re_register(&project.paths, &metadata).expect("the target configuration is complete"),
        "sbxm add example-org/example-repo --worktrees 3 --detach develop"
    );

    // 起点branchのないdetachedは再現できない。誤ったcommandを見せない。
    metadata.provisioning.start_ref = None;
    let error = re_register(&project.paths, &metadata)
        .expect_err("a configuration that cannot be repeated is not printed");
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
}

#[test]
fn a_cache_that_is_a_symlink_is_not_followed_and_the_project_stays_managed() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let elsewhere = fixture.workspace_root.parent().unwrap().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("keep.txt"), "not ours\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere, project.paths.cache_dir()).unwrap();

    let host = clean_host(&fixture, &project);
    host.listing.borrow_mut().insert(0, "[]".to_string());
    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let error = execute(&host, &prepared, poll()).expect_err("a symlinked cache is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(
        elsewhere.join("keep.txt").exists(),
        "what the link pointed at is untouched"
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so the state can be settled"
    );
}

/// 入力を決め打ちする確認prompt。`None`はEscまたはCtrl-C。
struct ScriptedConfirm {
    typed: Option<String>,
    asked: usize,
}

impl ScriptedConfirm {
    fn typing(value: &str) -> ScriptedConfirm {
        ScriptedConfirm {
            typed: Some(value.to_string()),
            asked: 0,
        }
    }

    fn canceling() -> ScriptedConfirm {
        ScriptedConfirm {
            typed: None,
            asked: 0,
        }
    }
}

impl ConfirmPrompt for ScriptedConfirm {
    fn confirm_sandbox_name(&mut self, expected: &str) -> Result<bool> {
        self.asked += 1;
        match &self.typed {
            Some(typed) => Ok(typed == expected),
            None => Err(Error::Canceled),
        }
    }
}

/// 削除して良い状態の案件を1件用意する。
fn prepared_project(fixture: &Fixture, force: bool) -> (FakeSbx, Prepared) {
    let project = fixture.register("example-org/example-repo");
    let host = clean_host(fixture, &project);
    host.listing.borrow_mut().insert(0, "[]".to_string());
    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        force,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");
    (host, prepared)
}

#[test]
fn only_the_exact_sandbox_name_confirms_an_interactive_deletion() {
    let fixture = fixture();
    let (_host, prepared) = prepared_project(&fixture, false);
    let sandbox = prepared.plan.sandbox.clone();

    let mut exact = ScriptedConfirm::typing(&sandbox);
    confirm(&prepared, true, &mut exact).expect("the name matched");
    assert_eq!(exact.asked, 1);

    // yesでは削除しない。名前以外の入力はすべて不一致とする。
    for answer in ["yes", "", &sandbox[..sandbox.len() - 1]] {
        let mut typed = ScriptedConfirm::typing(answer);
        let error = confirm(&prepared, true, &mut typed)
            .expect_err("only the sandbox name confirms a deletion");
        assert_eq!(error.first_id(), Some(ErrorId::DestroyNotConfirmed));
    }
}

#[test]
fn canceling_the_confirmation_changes_nothing_and_exits_130() {
    let fixture = fixture();
    let (host, prepared) = prepared_project(&fixture, false);

    let mut canceled = ScriptedConfirm::canceling();
    let error = confirm(&prepared, true, &mut canceled).expect_err("Esc and Ctrl-C leave");
    assert_eq!(error.exit_code(), ExitCode::Canceled);
    assert!(!host.ran("rm "), "nothing is removed");
}

#[test]
fn force_mode_and_a_non_interactive_run_are_not_asked_to_confirm() {
    let fixture = fixture();
    let (_host, normal) = prepared_project(&fixture, false);
    let mut without_terminal = ScriptedConfirm::canceling();
    confirm(&normal, false, &mut without_terminal)
        .expect("a fully specified project needs no prompt");
    assert_eq!(without_terminal.asked, 0);

    let forced_fixture = crate::testing::fixture();
    let (_host, forced) = prepared_project(&forced_fixture, true);
    let mut with_terminal = ScriptedConfirm::canceling();
    confirm(&forced, true, &mut with_terminal).expect("force mode skips the confirmation");
    assert_eq!(with_terminal.asked, 0);
}

/// `.sbxm`から書き込みを取り上げ、cleanupを失敗させる。
fn seal(paths: &ProjectPaths) -> std::path::PathBuf {
    let directory = paths
        .metadata_file()
        .parent()
        .expect("the metadata lives in a directory")
        .to_path_buf();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500))
        .expect("take away the write permission");
    directory
}

fn unseal(directory: &std::path::Path) {
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
        .expect("give it back so the temporary directory can be removed");
}

#[test]
fn a_cleanup_that_fails_before_the_commit_point_keeps_the_project_managed() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = clean_host(&fixture, &project);
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let sealed = seal(&project.paths);
    let error = execute(&host, &prepared, poll()).expect_err("the metadata cannot be removed");
    assert_eq!(error.first_id(), Some(ErrorId::CleanupFailed));
    assert!(
        project.paths.metadata_file().exists(),
        "the project is still managed, so destroy can be run again"
    );
    unseal(&sealed);
}

#[test]
fn a_lock_file_left_behind_is_a_warning_because_the_project_is_already_unmanaged() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = clean_host(&fixture, &project);
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    // metadataは消えており、lock fileだけが残せない状態を作る。
    std::fs::remove_file(project.paths.metadata_file()).unwrap();
    let sealed = seal(&project.paths);

    let outcome = execute(&host, &prepared, poll()).expect("the project is unmanaged already");
    assert_eq!(outcome.warnings.len(), 1, "the leftover is reported once");
    assert!(
        project.paths.lock_file().exists(),
        "the lock file is the thing that was left"
    );
    unseal(&sealed);
}

#[test]
fn a_sandbox_that_survives_its_removal_keeps_the_management_data() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // 削除後の一覧にも対象が残り続ける。
    let host = clean_host(&fixture, &project);

    let prepared = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let error = execute(&host, &prepared, poll()).expect_err("the sandbox is still there");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxStillPresent));
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so destroy can be run again"
    );
}
