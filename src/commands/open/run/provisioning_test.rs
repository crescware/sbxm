//! 初回構築まで進む`open`。
//!
//! 構築済みの案件を開く経路は`run_test`が持つ。ここでは、Sandboxをまだ持たない案件と、
//! 中断した初回構築が残っている案件だけを見る。

use crate::boundary::host::EnvPolicy;
use crate::commands::Context;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, SilentProgress, Ui};
use crate::diagnostics::{ErrorId, ExitCode, Result};
use crate::i18n::Locale;
use crate::paths::{PRIVATE_DIR_MODE, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout};
use crate::testing::add_request::request;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::poll::poll;
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};
use crate::testing::scripted_clock::ScriptedClock;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use super::{Prepared, prepare};

const PROJECT: &str = "Example-Org/Example-Repo";

/// 登録済み案件を`open`で開く。
fn open(bench: &Bench, world: &World, project: &ProjectId, index: Option<u32>) -> Result<Prepared> {
    let clock = ScriptedClock::default();
    prepare(
        &bench.location,
        &bench.config,
        Some(project),
        index,
        world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
        poll(&clock),
        &mut SilentProgress,
    )
}

/// `open`を入口から実行し、terminalへ出るはずのものを受け取る。
fn exec(
    bench: &Bench,
    world: &World,
    project: ProjectId,
    stdout: &mut Vec<u8>,
    stderr: &mut Vec<u8>,
) -> ExitCode {
    let clock = ScriptedClock::default();
    let policy = RenderingPolicy::plain();
    let mut ui = Ui::capture(Locale::En, policy, stdout, stderr);
    let mut prompt = PromptUi::new(
        Locale::En,
        policy.stderr,
        Box::new(ScriptedKeys::confirming()),
        Box::new(RecordedScreen::new()),
    );
    let context = Context {
        location: &bench.location,
        workspace_root: bench.workspace_root.path(),
        clock: &clock,
        locale: Locale::En,
        can_prompt: false,
    };
    crate::commands::open::exec(
        &crate::commands::open::Args {
            project: Some(project),
            index: None,
        },
        &context,
        &mut ui,
        world,
        &mut prompt,
    )
}

/// `add`だけを済ませた案件。
fn registered(bench: &Bench, world: &World, worktrees: Option<u32>) -> Checked<ProjectId> {
    // 2本以上のmanaged worktreeは、起点をbranchから切り離した案件だけが持てる。
    let detach = worktrees.filter(|count| *count >= 2).map(|_| "main");
    let request = request(PROJECT, worktrees, detach)?;
    bench
        .register(world, &request)
        .required_because("the project is registered")
}

#[test]
fn the_first_open_builds_the_sandbox_and_prepares_the_handover() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;

    let prepared = open(&bench, &world, &project, None)
        .required_because("the first open builds what it needs")?;

    let built = prepared
        .provisioned
        .as_ref()
        .required_because("the first open reports what it built")?;
    assert!(
        !built.already_built,
        "the first open builds instead of finding the project ready"
    );
    assert!(
        world.ran("sbx create"),
        "the sandbox is created in the same command: {:?}",
        world.invocations()
    );

    // 成果物を再観測できてからintentを消し、同じatomic replaceでbaselineを記録する。
    let stored = bench.stored(PROJECT)?;
    assert!(
        stored.initial_provisioning.is_none(),
        "a completed build leaves no repair intent"
    );
    let baseline = stored
        .declared_files
        .as_ref()
        .required_because("a completed build records the declared file baseline")?;
    assert_eq!(baseline.len(), 1);

    let layout = SandboxLayout::new(stored.canonical_id());
    assert_eq!(prepared.working_directory, layout.bare_root());
    assert_eq!(prepared.ssh_host, format!("{}.sbx", stored.sandbox_name()));
    Ok(())
}

#[test]
fn the_first_open_hands_the_root_size_environment_to_sbx_create() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;

    open(&bench, &world, &project, None).required_because("the first open builds")?;

    let create = world
        .calls
        .borrow()
        .iter()
        .find(|spec| spec.program == "sbx" && spec.args.first().is_some_and(|arg| arg == "create"))
        .cloned()
        .required_because("the first open creates the sandbox")?;
    assert_eq!(
        create.env,
        EnvPolicy::InheritWithoutSshAgent,
        "environment such as DOCKER_SANDBOXES_ROOT_SIZE must reach `sbx create` unfiltered"
    );
    Ok(())
}

#[test]
fn the_requested_worktree_index_is_honoured_on_the_first_build() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, Some(2))?;

    let prepared = open(&bench, &world, &project, Some(1))
        .required_because("the first open honours the selected index")?;

    let layout = SandboxLayout::new(bench.stored(PROJECT)?.canonical_id());
    assert_eq!(
        prepared.working_directory,
        format!("{}/{}", layout.bare_root(), layout.worktree_name(1))
    );
    assert_eq!(prepared.missing_worktree_index, None);
    Ok(())
}

#[test]
fn an_interrupted_first_open_is_resumed_by_the_next_open() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    world.failing("sbx create");

    open(&bench, &world, &project, None)
        .refused_because("the first provisioning mutation can fail")?;
    world.nothing_fails();
    assert!(
        bench.stored(PROJECT)?.initial_provisioning.is_some(),
        "the interrupted build leaves a repair intent"
    );

    let mark = world.mark();
    let prepared = open(&bench, &world, &project, None)
        .required_because("the next open resumes the interrupted build")?;

    assert!(prepared.provisioned.is_some());
    assert!(bench.stored(PROJECT)?.initial_provisioning.is_none());
    // 完成済みimageとTemplateを再作成せず、失敗したSandbox作成から続ける。
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("docker build")),
        "the completed image is reused: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn an_intentless_legacy_partial_build_is_completed_by_open() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    world.failing("sbx create");
    open(&bench, &world, &project, None).refused_because("the Sandbox creation is interrupted")?;
    world.nothing_fails();

    let mut legacy = bench.stored(PROJECT)?;
    legacy.initial_provisioning = None;
    crate::metadata::update(
        &ProjectPaths::derive(&bench.parent, &project.canonical()),
        &legacy,
    )
    .required_because("simulate metadata from before resumable intents")?;

    let mark = world.mark();
    open(&bench, &world, &project, None)
        .required_because("open completes the observable legacy partial build")?;
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("docker build")),
        "the verified image is reused: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn open_retargets_a_fixed_dockerfile_before_any_artifact_exists() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    world.failing("docker build");
    open(&bench, &world, &project, None).refused_because("the image build fails")?;
    world.nothing_fails();
    let original = bench.stored(PROJECT)?.provisioning.dockerfile_sha256;
    let paths = ProjectPaths::derive(&bench.parent, &project.canonical());
    fs::write(paths.dockerfile(), b"FROM example:fixed\n")
        .required_because("fix the Dockerfile")?;

    open(&bench, &world, &project, None)
        .required_because("open adopts the fixed input when the old generation has no artifact")?;

    let stored = bench.stored(PROJECT)?;
    assert_ne!(stored.provisioning.dockerfile_sha256, original);
    assert!(stored.initial_provisioning.is_none());
    Ok(())
}

#[test]
fn an_open_after_a_successful_build_connects_without_building_again() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    open(&bench, &world, &project, None).required_because("the first open builds")?;

    let mark = world.mark();
    let prepared =
        open(&bench, &world, &project, None).required_because("the built project opens")?;

    assert!(
        prepared.provisioned.is_none(),
        "a built project is not provisioned again"
    );
    let since = world.since(mark);
    assert!(
        !since
            .iter()
            .any(|call| call.contains("docker build") || call.contains("sbx create")),
        "nothing is built a second time: {since:?}"
    );
    Ok(())
}

#[test]
fn a_declared_file_edited_inside_the_sandbox_is_neither_refused_nor_placed_again() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    open(&bench, &world, &project, None).required_because("the first open builds")?;

    // 利用者がSandboxの中で設定fileを書き換えた。
    let destination = "/home/agent/.config/example/settings.yaml";
    let edited = crate::hash::sha256_hex(b"edited inside the sandbox\n");
    world
        .digests
        .borrow_mut()
        .insert(destination.to_string(), edited.clone());
    let mark = world.mark();

    open(&bench, &world, &project, None)
        .required_because("an edited file is not a broken artifact")?;

    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("exec -i")),
        "the edit is not overwritten: {:?}",
        world.since(mark)
    );
    assert_eq!(world.digests.borrow().get(destination), Some(&edited));
    Ok(())
}

#[test]
fn open_recreates_only_a_missing_managed_worktree() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, Some(2))?;
    open(&bench, &world, &project, None).required_because("the first open builds")?;

    let layout = SandboxLayout::new(bench.stored(PROJECT)?.canonical_id());
    let missing = layout.worktree(1);
    world.present.borrow_mut().remove(&missing);
    world.worktrees.borrow_mut().remove(&missing);
    let existing = layout.worktree(0);
    let existing_branch = world.worktrees.borrow().get(&existing).cloned();
    let mark = world.mark();

    let prepared = open(&bench, &world, &project, Some(1))
        .required_because("open restores the selected managed worktree")?;

    assert_eq!(prepared.working_directory, missing);
    assert_eq!(
        world.worktrees.borrow().get(&existing).cloned(),
        existing_branch
    );
    assert_eq!(
        world
            .since(mark)
            .iter()
            .filter(|call| call.contains("worktree add"))
            .count(),
        1,
        "only the missing worktree is created"
    );
    Ok(())
}

#[test]
fn open_recreates_a_missing_managed_repository_without_rebuilding_the_sandbox() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    open(&bench, &world, &project, None).required_because("the first open builds")?;

    let layout = SandboxLayout::new(bench.stored(PROJECT)?.canonical_id());
    world.present.borrow_mut().remove(&layout.bare_git_dir());
    for path in layout.worktree_names(1) {
        let absolute = format!("{}/{path}", layout.bare_root());
        world.present.borrow_mut().remove(&absolute);
        world.worktrees.borrow_mut().remove(&absolute);
    }
    world.repository.borrow_mut().clear();
    *world.bare_git_dir.borrow_mut() = None;
    let mark = world.mark();

    open(&bench, &world, &project, None)
        .required_because("open reconstructs the managed Git prerequisites")?;

    let calls = world.since(mark);
    assert!(calls.iter().any(|call| call.contains("git init --bare")));
    assert!(calls.iter().any(|call| call.contains("worktree add")));
    assert!(!calls.iter().any(|call| call.contains("sbx create")));
    assert!(!calls.iter().any(|call| call.contains("docker build")));
    Ok(())
}

#[test]
fn a_stopped_and_complete_project_is_started_instead_of_being_repaired() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    open(&bench, &world, &project, None).required_because("the first open builds")?;
    // 停止中は中を読めないが、それは欠落ではない。完成済みの案件をrepairへ送らない。
    world.stopped();

    let prepared = open(&bench, &world, &project, None)
        .required_because("a stopped project is started and opened")?;

    assert!(
        prepared.provisioned.is_none(),
        "a stopped complete project is not provisioned again"
    );
    assert_eq!(
        prepared.ssh_host,
        format!("{}.sbx", bench.stored(PROJECT)?.sandbox_name())
    );
    Ok(())
}

#[test]
fn the_first_open_reports_what_the_build_could_not_clean_up() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;

    // Template archiveを消せないまま構築が成功する状態を作る。構築の途中で起きた
    // 事実を、接続の前に見せずに済ませない。
    let cache = ProjectPaths::derive(&bench.parent, &project.canonical()).cache_dir();
    let sealed = cache.clone();
    world.mutate_before("template load", move || {
        let _ = fs::set_permissions(&sealed, fs::Permissions::from_mode(0o500));
    });

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = exec(&bench, &world, project, &mut stdout, &mut stderr);
    fs::set_permissions(&cache, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .required_because("the temporary project can be removed again")?;

    assert_eq!(code, ExitCode::Success);
    let stderr = String::from_utf8(stderr).required_because("open stderr is UTF-8")?;
    assert!(
        stderr.contains("archive"),
        "the build reports what it left behind: {stderr:?}"
    );
    assert!(
        stderr.contains("is built"),
        "the warning does not replace the report of what was built: {stderr:?}"
    );
    Ok(())
}

#[test]
fn a_failed_handover_after_a_successful_build_does_not_return_to_pending() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    // 構築は終わり、terminalの引き渡しだけが失敗する。
    world.failing("ssh -t");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = exec(&bench, &world, project, &mut stdout, &mut stderr);

    assert_ne!(code, ExitCode::Success);
    let stderr = String::from_utf8(stderr).required_because("open stderr is UTF-8")?;
    // stdoutはSSHへ渡すため、構築の報告はstderrへ出す。
    assert!(
        stderr.contains("is built"),
        "the build is reported before the handover: {stderr:?}"
    );
    assert!(
        stdout.is_empty(),
        "stdout is left to the session: {stdout:?}"
    );
    assert!(
        bench.stored(PROJECT)?.initial_provisioning.is_none(),
        "a handover failure does not put the finished build back into pending"
    );
    Ok(())
}

#[test]
fn a_local_project_is_saved_during_and_after_the_session() -> Checked {
    // 実物と同じ待ちの判断が、間隔ごとに保存を走らせる。端末へ何も書かずに保存を試みる。
    let bench = Bench::new()?;
    let world = World::new();
    let request = crate::commands::add::AddRequest {
        repository: crate::repository::RepositoryIdentity::local("/home/user/code/app/.git", "app")
            .required_because("a local repository")?,
        worktrees: None,
        detach: None,
        start_branch: Some("main".to_string()),
        parent_inside_repository: false,
    };
    bench.build(&world, &request).required()?;
    let mark = world.mark();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = exec(
        &bench,
        &world,
        ProjectId::parse("local/app").required()?,
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(
        code,
        ExitCode::Success,
        "{}",
        String::from_utf8_lossy(&stderr)
    );
    let calls = world.since(mark);
    let session = calls
        .iter()
        .position(|call| call.starts_with("ssh "))
        .required_because("the session starts")?;
    let saves = calls
        .iter()
        .skip(session)
        .filter(|call| call.contains(crate::support::host_sync::PLACE_SAVE_REFS))
        .count();
    assert_eq!(
        saves, 2,
        "once during and once after the session: {calls:?}"
    );
    Ok(())
}

#[test]
fn a_github_project_is_not_saved_to_the_host_around_the_session() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    open(&bench, &world, &project, None).required()?;
    let mark = world.mark();

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = exec(&bench, &world, project, &mut stdout, &mut stderr);

    assert_eq!(
        code,
        ExitCode::Success,
        "{}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains(crate::support::host_sync::PLACE_SAVE_REFS)),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

const HOST_KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5 host-key\n";

/// 一度開いて完成させた案件。
fn built(
    bench: &Bench,
    world: &World,
    worktrees: Option<u32>,
) -> Checked<(ProjectPaths, ProjectId)> {
    let project = registered(bench, world, worktrees)?;
    open(bench, world, &project, None).required_because("the first open builds")?;
    let paths = ProjectPaths::derive(&bench.parent, bench.stored(PROJECT)?.canonical_id());
    Ok((paths, project))
}

/// 2本目のmanaged worktreeだけが欠けた、intentを持たない案件。
fn missing_a_worktree(bench: &Bench, world: &World) -> Checked<(ProjectPaths, ProjectId)> {
    let built = built(bench, world, Some(2))?;
    let missing = SandboxLayout::new(bench.stored(PROJECT)?.canonical_id()).worktree(1);
    world.present.borrow_mut().remove(&missing);
    world.worktrees.borrow_mut().remove(&missing);
    Ok(built)
}

fn set_mode(path: &std::path::Path, mode: u32) -> Checked {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).required()
}

fn refused(bench: &Bench, world: &World, project: &ProjectId) -> Checked<Option<ErrorId>> {
    Ok(open(bench, world, project, None)
        .refused_because("open stops before the terminal is handed over")?
        .first_id())
}

#[test]
fn open_does_not_take_a_lock_it_cannot_trust() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = built(&bench, &world, None)?;
    set_mode(&paths.lock_file(), 0o644)?;

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    Ok(())
}

#[test]
fn open_does_not_connect_under_a_session_lease_it_cannot_trust() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = built(&bench, &world, None)?;
    set_mode(&paths.session_lease_file(), 0o644)?;

    let mark = world.mark();

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree list"))
    );
    Ok(())
}

#[test]
fn open_does_not_resume_a_build_under_a_session_lease_it_cannot_trust() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = registered(&bench, &world, None)?;
    world.failing("sbx create");
    open(&bench, &world, &project, None).refused_because("the build is interrupted")?;
    world.nothing_fails();
    let paths = ProjectPaths::derive(&bench.parent, bench.stored(PROJECT)?.canonical_id());
    set_mode(&paths.session_lease_file(), 0o644)?;
    let mark = world.mark();

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("sbx create"))
    );
    Ok(())
}

#[test]
fn open_does_not_restore_a_worktree_under_a_session_lease_it_cannot_trust() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = missing_a_worktree(&bench, &world)?;
    set_mode(&paths.session_lease_file(), 0o644)?;
    let mark = world.mark();

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree add"))
    );
    Ok(())
}

#[test]
fn open_stops_when_the_sandboxes_cannot_be_listed() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = built(&bench, &world, None)?;
    world.timing_out("sbx ls");

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandTimeout)
    );
    Ok(())
}

#[test]
fn open_does_not_guess_between_two_sandboxes_of_the_same_name() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = built(&bench, &world, None)?;
    let row = world.sandboxes.borrow().first().cloned();
    world.sandboxes.borrow_mut().extend(row);

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::SandboxNameCollision)
    );
    Ok(())
}

#[test]
fn a_stopped_sandbox_whose_workspace_became_a_symlink_is_not_started() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = built(&bench, &world, None)?;
    world.stopped();
    let workspace = bench
        .workspace_root
        .path()
        .join(bench.stored(PROJECT)?.sandbox_name().as_str());
    let elsewhere = bench.workspace_root.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).required()?;
    fs::remove_dir_all(&workspace).required()?;
    std::os::unix::fs::symlink(&elsewhere, &workspace).required()?;

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ProjectPathSymlink)
    );
    assert!(!world.ran("/bin/true"));
    Ok(())
}

/// 初回構築がworktreeの手前で止まり、そのあとSandboxも止まった案件。
fn interrupted_and_stopped(bench: &Bench, world: &World) -> Checked<ProjectId> {
    let project = registered(bench, world, None)?;
    world.failing("worktree add");
    open(bench, world, &project, None).refused_because("the build is interrupted")?;
    world.nothing_fails();
    world.stopped();
    Ok(project)
}

#[test]
fn an_interrupted_build_whose_sandbox_does_not_start_is_not_resumed() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = interrupted_and_stopped(&bench, &world)?;
    world.failing("/bin/true");
    let mark = world.mark();

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandFailed)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree add"))
    );
    Ok(())
}

#[test]
fn an_interrupted_build_whose_sandbox_is_not_seen_running_is_not_resumed() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = interrupted_and_stopped(&bench, &world)?;
    world.change_before("/bin/true", |world| world.timing_out("sbx ls"));
    let mark = world.mark();

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandTimeout)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree add"))
    );
    Ok(())
}

#[test]
fn a_stopped_project_that_does_not_start_is_not_opened() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = built(&bench, &world, None)?;
    world.stopped();
    world.failing("/bin/true");

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandFailed)
    );
    Ok(())
}

#[test]
fn a_missing_worktree_is_not_restored_from_an_unfinished_observation() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = missing_a_worktree(&bench, &world)?;
    // 1回目は一覧、2回目は起動の確認、3回目が欠落の観測である。
    world.change_before("sbx ls", |world| {
        world.change_before("sbx ls", |world| {
            world.change_before("sbx ls", |world| world.timing_out("sbx ls"));
        });
    });
    let mark = world.mark();

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandTimeout)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree add"))
    );
    Ok(())
}

#[test]
fn a_missing_worktree_is_not_restored_from_an_unreadable_record() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = missing_a_worktree(&bench, &world)?;
    set_mode(&paths.snapshot_dir(), 0o000)?;
    let mark = world.mark();

    let id = refused(&bench, &world, &project)?;
    set_mode(&paths.snapshot_dir(), 0o700)?;

    assert_eq!(id, Some(ErrorId::ProjectPathUnreadable));
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree add"))
    );
    Ok(())
}

#[test]
fn a_missing_worktree_that_cannot_be_restored_stops_open() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = missing_a_worktree(&bench, &world)?;
    world.failing("worktree add");

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandFailed)
    );
    Ok(())
}

#[test]
fn a_restored_worktree_whose_completion_cannot_be_observed_stops_open() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = missing_a_worktree(&bench, &world)?;
    world.change_before("worktree add", |world| {
        world.change_before("sbx ls", |world| world.timing_out("sbx ls"));
    });

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandTimeout)
    );
    Ok(())
}

#[test]
fn a_restored_worktree_whose_completion_is_unsafe_stops_open() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = missing_a_worktree(&bench, &world)?;
    world.change_before("worktree add", |world| {
        world.answering("ssh-add -L", 0, HOST_KEY);
    });

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::SshAgentExposed)
    );
    Ok(())
}

#[test]
fn a_worktree_lost_again_after_the_restoration_stops_open() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = missing_a_worktree(&bench, &world)?;
    let missing = SandboxLayout::new(bench.stored(PROJECT)?.canonical_id()).worktree(1);
    world.change_before("worktree add", move |world| {
        let missing = missing.clone();
        world.change_before("sbx ls", move |world| {
            world.present.borrow_mut().remove(&missing);
            world.worktrees.borrow_mut().remove(&missing);
        });
    });

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::InitialProvisioningIncomplete)
    );
    Ok(())
}

#[test]
fn a_host_agent_that_appears_after_the_observation_stops_the_handover() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = built(&bench, &world, None)?;
    // 1回目は観測の中、2回目が接続の直前の確認である。
    world.change_before("printenv SSH_AUTH_SOCK", |world| {
        world.change_before("printenv SSH_AUTH_SOCK", |world| {
            world.answering("ssh-add -L", 0, HOST_KEY);
        });
    });

    let mark = world.mark();

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::SshAgentExposed)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree list"))
    );
    Ok(())
}

#[test]
fn open_stops_when_the_worktrees_cannot_be_listed() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = built(&bench, &world, None)?;
    world.failing("worktree list");

    assert_eq!(
        refused(&bench, &world, &project)?,
        Some(ErrorId::ExternalCommandFailed)
    );
    Ok(())
}

/// 登録済み案件を、任意のhostを通して`open`で開く。
fn opened(
    bench: &Bench,
    host: &dyn crate::boundary::host::HostEnvironment,
    project: &ProjectId,
) -> Result<Prepared> {
    let clock = ScriptedClock::default();
    prepare(
        &bench.location,
        &bench.config,
        Some(project),
        None,
        host,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
        poll(&clock),
        &mut SilentProgress,
    )
}

/// `arrange`が整えた案件を開く起動を1つずつ時間切れにし、どれが答えなくても端末を
/// 渡さないことを確かめる。
fn no_unanswered_step_hands_over_the_terminal(
    arrange: impl Fn(&Bench, &World) -> Checked<ProjectId>,
) -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = arrange(&bench, &world)?;
    let recorded = crate::testing::host::FailingAt::recording(world);
    opened(&bench, &recorded, &project).required_because("every step answers")?;
    for (at, step) in recorded.calls().iter().enumerate() {
        // 空き容量は、読めなかった理由として接続の前に示す。接続そのものは止めない。
        if step.contains("df -Pk") {
            continue;
        }
        let bench = Bench::new()?;
        let world = World::new();
        let project = arrange(&bench, &world)?;
        let failing = crate::testing::host::FailingAt::timing_out(world, at);
        assert!(opened(&bench, &failing, &project).is_err(), "{step}");
    }
    Ok(())
}

#[test]
fn opening_a_built_project_stops_at_any_step_that_does_not_answer() -> Checked {
    no_unanswered_step_hands_over_the_terminal(|bench, world| Ok(built(bench, world, None)?.1))
}

#[test]
fn restoring_a_missing_worktree_stops_at_any_step_that_does_not_answer() -> Checked {
    no_unanswered_step_hands_over_the_terminal(|bench, world| {
        Ok(missing_a_worktree(bench, world)?.1)
    })
}

#[test]
fn starting_a_stopped_project_stops_at_any_step_that_does_not_answer() -> Checked {
    no_unanswered_step_hands_over_the_terminal(|bench, world| {
        let (_, project) = built(bench, world, None)?;
        world.stopped();
        Ok(project)
    })
}

#[test]
fn resuming_an_interrupted_build_stops_at_any_step_that_does_not_answer() -> Checked {
    no_unanswered_step_hands_over_the_terminal(|bench, world| {
        let project = registered(bench, world, None)?;
        world.failing("worktree add");
        open(bench, world, &project, None).refused_because("the build is interrupted")?;
        world.nothing_fails();
        Ok(project)
    })
}
