//! 初回構築まで進む`open`。
//!
//! 構築済みの案件を開く経路は`run_test`が持つ。ここでは、Sandboxをまだ持たない案件と、
//! 中断した初回構築が残っている案件だけを見る。

use crate::boundary::host::EnvPolicy;
use crate::commands::Context;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, SilentProgress, Ui};
use crate::diagnostics::{ExitCode, Result};
use crate::i18n::Locale;
use crate::paths::{PRIVATE_DIR_MODE, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout};
use crate::testing::add_request::request;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::poll::poll;
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use super::{Prepared, prepare};

const PROJECT: &str = "Example-Org/Example-Repo";

/// 登録済み案件を`open`で開く。
fn open(bench: &Bench, world: &World, project: &ProjectId, index: Option<u32>) -> Result<Prepared> {
    prepare(
        &bench.location,
        &bench.config,
        Some(project),
        index,
        world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
        poll(),
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
