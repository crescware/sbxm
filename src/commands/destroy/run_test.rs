use super::*;
use crate::command::{EnvPolicy, OutputPolicy};
use crate::error::{ErrorId, ExitCode};
use crate::metadata;
use crate::paths::PRIVATE_FILE_MODE;
use crate::testing::host::{
    FakeSbx, assert_lifecycle, custom_secret_listing, no_custom_secrets, no_secrets,
};
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, Registered, fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::protection::clean_host;
use crate::ui::SilentProgress;
use std::os::unix::fs::PermissionsExt;

fn path_of(target: &Target) -> Option<&str> {
    match target {
        Target::Path(path) => Some(path.as_str()),
        Target::Described(_) => None,
    }
}

/// 説明で示す対象が計画に並んでいるか。
fn describes(plan: &DestroyPlan, id: &str) -> bool {
    plan.removes
        .iter()
        .chain(plan.keeps.iter())
        .any(|target| matches!(target, Target::Described(message) if message.id == id))
}

/// hostへ渡った指定の順番。工程の前後関係を検証する。
fn position(host: &FakeSbx, needle: &str) -> usize {
    host.calls()
        .iter()
        .position(|args| args.join(" ").contains(needle))
        .unwrap_or_else(|| panic!("no command matched {needle}"))
}

#[test]
fn a_clean_running_project_is_planned_then_removed() {
    let fixture = fixture();
    let project = fixture.register("Example-Org/Example-Repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    std::fs::create_dir_all(project.paths.cache_dir()).unwrap();
    // 削除後の一覧では対象が消えている。
    let host = no_secrets(clean_host(&fixture, &project), project.sandbox.as_str());
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.location,
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
            .any(|target| path_of(target).is_some_and(|path| path.contains("project.yaml")))
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
        "sbxm add git@github.com:Example-Org/Example-Repo.git --worktrees 1"
    );

    let outcome = execute(&host, &prepared, poll(), &mut SilentProgress).expect("destroy");
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
    let host = no_secrets(clean_host(&fixture, &project), project.sandbox.as_str());
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");
    execute(&host, &prepared, poll(), &mut SilentProgress).expect("destroy");

    assert_lifecycle(&host, &format!("rm --force {}", project.sandbox));

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
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect_err("a stopped sandbox cannot be inspected");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));

    let host = no_secrets(
        FakeSbx::listings(&[&stopped, "[]"]),
        project.sandbox.as_str(),
    );
    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        true,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("force skips the checks");
    assert!(prepared.plan.force);
    assert!(prepared.plan.worktrees.is_empty());

    execute(&host, &prepared, poll(), &mut SilentProgress).expect("destroy");
    // `--force`は`sbx`の確認promptを省くためのもので、常に付ける。sbxm側の
    // `--force`はsbxm自身のデータ保護検査を省くことを指す。
    assert!(host.ran(&format!("rm --force {}", project.sandbox)));
    assert!(!project.paths.metadata_file().exists());
}

#[test]
fn unsaved_work_stops_the_normal_mode_before_anything_is_deleted() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let host = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "1 .M N... 100644 100644 100644 abc abc file.txt\0",
    );

    let error = prepare(
        &fixture.location,
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
fn an_unmanaged_project_is_refused_before_the_host_is_touched() {
    let fixture = fixture();
    let host = FakeSbx::listing("[]");

    let error = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect_err("a project that is not managed has nothing to destroy");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    assert!(
        host.calls().is_empty(),
        "the host is not asked anything before the target is decided: {:?}",
        host.calls()
    );
}

#[test]
fn a_project_without_a_sandbox_only_loses_its_management_data() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = no_secrets(FakeSbx::listing("[]"), project.sandbox.as_str());

    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");
    assert_eq!(prepared.plan.state, ProjectState::NotCreated);

    execute(&host, &prepared, poll(), &mut SilentProgress).expect("destroy");
    assert!(!host.ran("rm "), "there is no sandbox to remove");
    assert!(!project.paths.metadata_file().exists());
}

#[test]
fn the_token_registration_goes_away_with_the_sandbox() {
    // scopeはSandboxの有無と無関係に残る。登録を残したまま管理を解くと、次に同じ案件を
    // addして案内どおりにset-customを実行しても、同じenvの登録が既にあるとして拒否される。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let sandbox = project.sandbox.as_str();
    let host = clean_host(&fixture, &project).answering_in_turn(
        &format!("secret ls {sandbox}"),
        &[
            (0, &custom_secret_listing(sandbox, "sbx-cs-example")),
            (0, &no_custom_secrets(sandbox)),
        ],
    );
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");
    // 消す前に、tokenの登録も消えることを見せる。
    assert!(describes(&prepared.plan, "destroy-target-secret"));

    execute(&host, &prepared, poll(), &mut SilentProgress).expect("destroy");
    assert!(
        host.ran(&format!(
            "secret rm {sandbox} --placeholder sbx-cs-example --force"
        )),
        "the registration is named by its placeholder: {:?}",
        host.calls()
    );
    // 消し損ねたSandboxがplaceholderを持ったまま残る順序にしない。
    assert!(
        position(&host, &format!("rm --force {sandbox}")) < position(&host, "secret rm"),
        "the sandbox goes first, then the token it was given"
    );
    assert!(!project.paths.metadata_file().exists());
}

#[test]
fn a_registration_that_survives_its_removal_keeps_the_project_managed() {
    // commandの戻り値だけを不在の根拠にしない。残ったままなら、同じcommandでやり直せる
    // 状態を保ったまま停止する。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let sandbox = project.sandbox.as_str();
    let host = clean_host(&fixture, &project).answering(
        &format!("secret ls {sandbox}"),
        0,
        &custom_secret_listing(sandbox, "sbx-cs-example"),
    );
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let error = execute(&host, &prepared, poll(), &mut SilentProgress)
        .expect_err("the registration is still listed");
    assert_eq!(error.first_id(), Some(ErrorId::SecretStillRegistered));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .expect("the user is told how to remove it");
    assert!(
        remediation.commands.iter().any(|command| command.as_str()
            == format!("sbx secret rm {sandbox} --placeholder sbx-cs-example --force")),
        "the command that removes it is its own line: {remediation:?}"
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so destroy can be run again"
    );
}

#[test]
fn a_registration_of_another_scope_is_left_to_the_sandboxes_that_use_it() {
    // global scopeのsecretはほかのSandboxも使う。1案件の後片付けで消す対象ではない。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let sandbox = project.sandbox.as_str();
    let host = clean_host(&fixture, &project).answering(
        &format!("secret ls {sandbox}"),
        0,
        &custom_secret_listing("global", "sbx-cs-elsewhere"),
    );
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    execute(&host, &prepared, poll(), &mut SilentProgress).expect("destroy");
    assert!(
        !host.ran("secret rm"),
        "a registration this project did not own is untouched: {:?}",
        host.calls()
    );
    // ほかのSandbox向けのsecretは保持対象として示す。
    assert!(describes(&prepared.plan, "destroy-target-secrets"));
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
        "sbxm add git@github.com:example-org/example-repo.git --worktrees 3 --detach develop"
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

    let host = no_secrets(clean_host(&fixture, &project), project.sandbox.as_str());
    host.listing.borrow_mut().insert(0, "[]".to_string());
    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let error = execute(&host, &prepared, poll(), &mut SilentProgress)
        .expect_err("a symlinked cache is refused");
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
    let host = no_secrets(clean_host(fixture, &project), project.sandbox.as_str());
    host.listing.borrow_mut().insert(0, "[]".to_string());
    let prepared = prepare(
        &fixture.location,
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

    let forced_fixture = crate::testing::project::fixture();
    let (_host, forced) = prepared_project(&forced_fixture, true);
    let mut with_terminal = ScriptedConfirm::canceling();
    confirm(&forced, true, &mut with_terminal).expect("force mode skips the confirmation");
    assert_eq!(with_terminal.asked, 0);
}

#[test]
fn the_project_lock_is_held_across_the_confirmation() {
    let fixture = fixture();
    let (_host, prepared) = prepared_project(&fixture, false);
    let lock_file = ProjectPaths::derive(
        &fixture.parent,
        &project_id("example-org/example-repo").canonical(),
    )
    .lock_file();

    // 確認を待つあいだも、別の実行はこの案件へ入れない。
    let waiting = std::time::Duration::from_millis(200);
    paths::acquire_exclusive_lock(
        &lock_file,
        waiting,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect_err("another run cannot reach the project while the confirmation is pending");

    // lockはPreparedとともに解放される。
    drop(prepared);
    paths::acquire_exclusive_lock(
        &lock_file,
        waiting,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect("the lock is released with Prepared");
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
    let host = no_secrets(clean_host(&fixture, &project), project.sandbox.as_str());
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let sealed = seal(&project.paths);
    let error = execute(&host, &prepared, poll(), &mut SilentProgress)
        .expect_err("the metadata cannot be removed");
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
    let host = no_secrets(clean_host(&fixture, &project), project.sandbox.as_str());
    host.listing.borrow_mut().insert(0, "[]".to_string());

    let prepared = prepare(
        &fixture.location,
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

    let outcome = execute(&host, &prepared, poll(), &mut SilentProgress)
        .expect("the project is unmanaged already");
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
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let error = execute(&host, &prepared, poll(), &mut SilentProgress)
        .expect_err("the sandbox is still there");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxStillPresent));
    assert!(
        !host.ran("secret rm"),
        "a sandbox that still exists keeps the token it was given"
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so destroy can be run again"
    );
}

/// 管理解除をcommitした案件として、registryから外す要求を組み立てる。
fn unregistration_of(project: &Registered) -> Unregistration {
    Unregistration {
        paths: project.paths.clone(),
        repository: project.metadata.repository.clone(),
    }
}

#[test]
fn unregistering_removes_the_entry_of_that_project_and_no_other() {
    let fixture = fixture();
    let target = fixture.register("example-org/example-repo");
    let other = fixture.register("other-org/other-repo");
    // metadataの削除が管理解除のcommit pointである。
    std::fs::remove_file(target.paths.metadata_file()).expect("remove the metadata");

    assert!(
        unregister(&fixture.location, &unregistration_of(&target))
            .expect("unregister")
            .is_none()
    );

    let registry = crate::registry::load(&fixture.location).expect("the registry stays valid");
    assert_eq!(
        registry
            .entries()
            .iter()
            .map(|entry| entry.canonical_id().as_str())
            .collect::<Vec<_>>(),
        vec!["other-org/other-repo"]
    );
    assert!(
        other.paths.metadata_file().exists(),
        "another project is untouched"
    );
    // 利用者の成果物は残す。registryから外れたあとは未登録のhost artifactとして扱う。
    assert!(target.paths.root().is_dir());
}

#[test]
fn a_project_registered_again_during_the_removal_keeps_its_entry() {
    // destroyはmetadataを消してproject lockを手放したあとでregistry lockを取る。その
    // 隙間に同じ案件のaddが登録をやり直していれば、entryは新しい登録意図を指している。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = no_secrets(clean_host(&fixture, &project), project.sandbox.as_str());
    host.listing.borrow_mut().insert(0, "[]".to_string());
    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .expect("prepare");

    let unregistration = prepared.unregistration();
    execute(&host, &prepared, poll(), &mut SilentProgress).expect("destroy");
    drop(prepared);

    // project lockが空いているあいだに、別の実行のaddがmetadataを作り直した。
    metadata::create(&project.paths, &project.metadata).expect("register the project again");

    let warning = unregister(&fixture.location, &unregistration)
        .expect("unregister")
        .expect("the re-registration is reported");
    assert_eq!(warning.description.id, "warning-project-registered-again");
    let registry = crate::registry::load(&fixture.location).expect("the registry stays valid");
    assert!(
        registry.find(project.metadata.canonical_id()).is_some(),
        "the project that was registered again stays discoverable"
    );
}

#[test]
fn an_entry_that_names_another_root_is_left_to_the_run_that_recorded_it() {
    // 同じcanonical project IDでも、別のrootを指すentryはこの実行が記録したものではない。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let unregistration = unregistration_of(&project);
    std::fs::remove_file(project.paths.metadata_file()).expect("remove the metadata");

    // 別の実行が、entryを外して別のrootへ登録し直した。
    let mut guard = RegistryGuard::acquire(&fixture.location).expect("acquire the registry lock");
    guard
        .remove(project.metadata.canonical_id())
        .expect("remove the entry");
    drop(guard);
    let elsewhere = fixture._dir.path().join("elsewhere.project");
    std::fs::create_dir_all(&elsewhere).expect("create the other root");
    fixture.record(&elsewhere, project.metadata.repository.clone());

    let warning = unregister(&fixture.location, &unregistration)
        .expect("unregister")
        .expect("an entry another run recorded is reported");
    assert_eq!(warning.description.id, "warning-project-registered-again");
    let registry = crate::registry::load(&fixture.location).expect("the registry stays valid");
    assert_eq!(
        registry
            .find(project.metadata.canonical_id())
            .expect("the entry stays")
            .project_root(),
        elsewhere,
        "the registration of the other run is untouched"
    );
}

#[test]
fn a_root_that_cannot_be_verified_keeps_the_entry_instead_of_guessing() {
    // 再登録の有無を観測できなければ、管理解除が維持されているとみなさない。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let unregistration = unregistration_of(&project);
    std::fs::remove_dir_all(project.paths.root()).expect("remove the project root");
    std::fs::write(project.paths.root(), b"not a project root\n").expect("put a file in its place");

    let error = unregister(&fixture.location, &unregistration)
        .expect_err("a root that is not a directory cannot be verified");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    let registry = crate::registry::load(&fixture.location).expect("the registry stays valid");
    assert!(
        registry.find(project.metadata.canonical_id()).is_some(),
        "the entry stays until the state can be observed"
    );
}

#[test]
fn an_entry_left_behind_by_a_crash_after_the_commit_point_is_tolerated() {
    // metadata削除後、registry entry削除前に中断すると、不整合entryが残り得る。
    // 通常modeで推測して消さず、read-onlyの観測へ残す。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::remove_file(project.paths.metadata_file()).expect("remove the metadata");

    let registry = crate::registry::load(&fixture.location).expect("the registry stays valid");
    assert_eq!(registry.entries().len(), 1);

    let error = select::locked(&fixture.location, &project_id("example-org/example-repo"))
        .expect_err("the project cannot be worked on");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectIncomplete));
}
