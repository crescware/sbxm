use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::design::SilentProgress;
use crate::diagnostics::{ErrorId, Result};
use crate::metadata;
use crate::paths::{self, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};
use std::cell::Cell;
use std::fs;
use std::path::PathBuf;

use crate::project::{ProjectId, SandboxLayout};

use super::{Prepared, RepairAction, execute, prepare};
use crate::commands::repair::RepairOutput;

struct StateChangingHost<'a> {
    world: &'a World,
    workspace: PathBuf,
    sandbox_listings: Cell<usize>,
}

impl HostEnvironment for StateChangingHost<'_> {
    fn command_exists(&self, program: &str) -> bool {
        self.world.command_exists(program)
    }

    fn run(&self, spec: &CommandSpec) -> crate::diagnostics::Result<CommandOutcome> {
        if spec.program == "sbx" && spec.args.as_slice() == ["ls", "--json"] {
            let listing = self.sandbox_listings.get();
            self.sandbox_listings.set(listing + 1);
            if listing == 1 {
                self.world.images.borrow_mut().clear();
                self.world.templates.borrow_mut().clear();
                self.world.sandboxes.borrow_mut().clear();
                if self.workspace.is_dir() {
                    assert!(fs::remove_dir_all(&self.workspace).is_ok());
                }
            }
        }
        self.world.run(spec)
    }
}

#[test]
fn repair_remains_a_compatible_entry_for_an_interrupted_open() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("docker build");

    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;
    let project = project_of(&request)?;
    let error = bench
        .ensure(&world, &project, &mut SilentProgress)
        .refused_because("the failed first mutation leaves an intent")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some()
    );
    world.nothing_fails();

    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair prepares an explicit plan")?;
    assert_eq!(prepared.plan.state.as_str(), "pending");
    let output = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair completes the remaining provisioning")?;
    assert!(output.changed);
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none()
    );
    Ok(())
}

#[test]
fn repair_uses_the_recorded_snapshot_after_global_input_changes() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("docker build");
    bench
        .build(&world, &request)
        .refused_because("the first build is interrupted")?;
    world.nothing_fails();
    fs::write(bench.config.files[0].source.as_path(), b"changed = true\n")
        .required_because("change the global file input")?;

    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair plans from the recorded input")?;
    let output = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair resumes from the immutable snapshot")?;
    assert!(output.changed);
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none()
    );
    Ok(())
}

#[test]
fn an_active_session_blocks_repair_without_evicting_it() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("docker build");
    bench
        .build(&world, &request)
        .refused_because("the first build is interrupted")?;
    world.nothing_fails();

    let project = project_of(&request)?;
    let session = paths::acquire_shared_lock(
        &crate::paths::ProjectPaths::derive(&bench.parent, &project.canonical())
            .session_lease_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("simulate an active remote session")?;
    let mark = world.mark();
    let error = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("repair must not evict an active session")?;
    assert_eq!(error.first_id(), Some(ErrorId::OpenSessionActive));
    assert!(
        !world.since(mark).iter().any(|call| {
            call.contains("docker build")
                || call.contains("sbx create")
                || call.contains("template load")
        }),
        "the active session is refused before mutation: {:?}",
        world.since(mark)
    );
    drop(session);
    Ok(())
}

#[test]
fn repair_rechecks_state_after_taking_the_exclusive_lease() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the old run stopped after reusable artifacts were made")?;
    world.nothing_fails();

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    stored.initial_provisioning = None;
    metadata::update(&paths, &stored).required_because("make the record legacy")?;

    let sandbox = stored.sandbox_name().to_string();
    let host = StateChangingHost {
        world: &world,
        workspace: bench.workspace_root.path().join(&sandbox),
        sandbox_listings: Cell::new(0),
    };
    let error = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &host,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("repair does not apply a stale plan")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningStateChanged)
    );
    Ok(())
}

#[test]
fn an_interrupted_build_is_still_repairable_after_its_sandbox_stopped() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the build is interrupted after the sandbox exists")?;
    world.nothing_fails();

    // 中断した案件のSandboxが止まっていても、保存済みintentが復旧先を固定している。
    // 観測できない状態を理由にrepairを閉ざさない。
    world.stopped();
    let project = project_of(&request)?;
    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("a saved intent still names what to recover")?;
    assert_eq!(prepared.plan.state.as_str(), "pending");
    assert_eq!(
        prepared.plan.actions,
        vec![
            RepairAction::ReuseImage,
            RepairAction::ReuseTemplate,
            RepairAction::StartSandbox,
            RepairAction::ProvisionInterior,
            RepairAction::ClearIntent,
        ],
        "the plan names the implicit start and full interior provisioning without guessing gaps"
    );

    let output = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair finishes the interrupted build")?;
    assert!(output.changed);

    let stored = bench.stored("Example-Org/Example-Repo")?;
    assert!(
        stored.initial_provisioning.is_none(),
        "a verified repair clears the intent"
    );
    Ok(())
}

#[test]
fn a_stopped_project_is_refused_rather_than_repaired_from_unread_facts() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first prepare succeeds")?;

    // 完成した案件のSandboxが止まっただけ。中を読めない以上、欠けた工程も導けない。
    world.stopped();
    let mark = world.mark();
    let error = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("repair does not plan from facts it could not observe")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningUnobservable)
    );
    let calls = world.since(mark);
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("exec") || call.contains("sbx start")),
        "{calls:?}"
    );
    Ok(())
}

#[test]
fn repair_is_read_only_for_fresh_and_ready_projects() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;
    let project = project_of(&request)?;

    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair observes a fresh project")?;
    assert_eq!(prepared.plan.state.as_str(), "fresh");
    let output = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("a fresh project needs no repair")?;
    assert!(!output.changed);

    bench
        .ensure(&world, &project, &mut SilentProgress)
        .required_because("the normal prepare finishes the project")?;

    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair observes a ready project")?;
    assert_eq!(prepared.plan.state.as_str(), "ready");
    let output = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("a ready project needs no repair")?;
    assert!(!output.changed);
    Ok(())
}

#[test]
fn a_legacy_incomplete_project_records_intent_before_repairing() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the old run stopped after reusable artifacts were made")?;
    world.nothing_fails();

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    let generation = stored.provisioning.dockerfile_sha256.clone();
    stored.initial_provisioning = None;
    metadata::update(&paths, &stored).required_because("remove the legacy-less intent")?;
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("change the Dockerfile after the old interruption")?;

    let project = project_of(&request)?;
    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair can select the stored generation")?;
    assert_eq!(prepared.plan.state.as_str(), "incomplete");
    assert_eq!(prepared.target, generation);
    let mark = world.mark();
    let output = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair records and completes a legacy partial build")?;
    assert!(output.changed);

    let stored = bench.stored("Example-Org/Example-Repo")?;
    assert!(stored.initial_provisioning.is_none());
    assert_eq!(stored.provisioning.dockerfile_sha256, generation);
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("docker build"))
    );
    Ok(())
}

#[test]
fn repair_stops_when_a_legacy_plan_is_no_longer_true() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the old run stopped after reusable artifacts were made")?;
    world.nothing_fails();

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    stored.initial_provisioning = None;
    metadata::update(&paths, &stored).required_because("make the record legacy")?;
    let project = project_of(&request)?;
    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair prepares the legacy plan")?;
    let mark = world.mark();

    let sandbox = world.sandboxes.borrow()[0].name.clone();
    let workspace = bench.workspace_root.path().join(&sandbox);
    world.images.borrow_mut().clear();
    world.templates.borrow_mut().clear();
    world.sandboxes.borrow_mut().clear();
    fs::remove_dir_all(&workspace).required_because("remove the planned workspace")?;

    let error = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .refused_because("repair rechecks its state before mutation")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningStateChanged)
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("docker build"))
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
fn repair_clears_an_intent_after_read_only_completion_verification() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the project is complete")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    let inputs =
        crate::support::provisioning::ProvisioningInputs::capture(&paths, &bench.config, None)
            .required_because("capture the current inputs as a snapshot")?;
    let intent = crate::support::provisioning::initial_intent(&inputs);
    stored.initial_provisioning = Some(intent.clone());
    metadata::update(&paths, &stored).required_because("persist the already-complete intent")?;
    fs::remove_dir_all(paths.snapshot_dir())
        .required_because("remove inputs that a completed sandbox no longer needs")?;

    let mark = world.mark();
    let project = project_of(&request)?;
    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair verifies the pending completed state")?;
    let output = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair clears only the intent")?;
    assert!(output.changed);
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none()
    );
    assert_eq!(
        bench.stored("Example-Org/Example-Repo")?.declared_files,
        Some(intent.files),
        "completion records the baseline while clearing the intent"
    );
    assert!(
        !world.since(mark).iter().any(|call| {
            call.contains("docker build") || call.contains("sbx create") || call.contains("sbx cp")
        }),
        "a complete pending project is only observed and cleared: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn repair_does_not_clear_an_intent_it_cannot_verify_as_complete() -> Checked {
    // 完成確認そのものが観測できなければ、成果物が揃って見えても完了とみなさない。
    // intentを消すのは、全post-conditionをread-onlyで確認できたときだけである。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the project is complete")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    let inputs =
        crate::support::provisioning::ProvisioningInputs::capture(&paths, &bench.config, None)
            .required_because("capture the current inputs as a snapshot")?;
    stored.initial_provisioning = Some(crate::support::provisioning::initial_intent(&inputs));
    metadata::update(&paths, &stored).required_because("persist the already-complete intent")?;

    world.failing_with("rev-parse HEAD", "fatal: not a git repository\n");
    let project = project_of(&request)?;
    prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("an unreadable worktree head is not read as a completed post-condition")?;

    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some(),
        "the intent is not cleared while completion cannot be verified"
    );
    Ok(())
}

#[test]
fn repair_asks_for_another_repair_when_completion_cannot_be_reverified_after_provisioning()
-> Checked {
    // provisionそのものは成功しても、直後の読み取り専用の完成確認がそれを裏付けられ
    // なければ、intentは消さない。次のrepairへ安全に送る。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the old run stopped after reusable artifacts were made")?;
    world.nothing_fails();

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let workspace = bench
        .workspace_root
        .path()
        .join(stored.sandbox_name().as_str());
    let project = project_of(&request)?;
    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair prepares an explicit plan")?;

    // 最後のworktreeを作った直後、中立workspaceが消える。provisionそのものは成功
    // で終わるが、直後の再観測はそれを完成として確認できない。
    world.mutate_before("worktree add", move || {
        let _ = fs::remove_dir_all(&workspace);
    });
    let error = execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .refused_because("provisioning that cannot be reverified as complete is not a success")?;
    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some(),
        "the intent is not cleared when completion cannot be reverified"
    );
    Ok(())
}

#[test]
fn an_interrupted_local_project_is_repaired_without_a_token() -> Checked {
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
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the first build stopped before the worktree")?;
    world.nothing_fails();

    let project = crate::project::ProjectId::parse("local/app").required()?;
    let prepared = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("an interrupted local build is repairable")?;
    // hostから送るrepositoryには、tokenを使うhelperが無い。
    assert!(
        prepared
            .plan
            .observations
            .iter()
            .all(|field| field.label.id != "repair-observation-credential-helper"),
        "{:?}",
        prepared.plan.observations
    );
    let mark = world.mark();
    execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair completes the local build")?;
    assert!(
        !world.since(mark).iter().any(|call| call.contains("secret")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

/// 古い版の構築がworktreeの手前で止まり、intentを持たない途中の状態として残った案件。
fn legacy_incomplete(bench: &Bench, world: &World) -> Checked<(ProjectPaths, ProjectId)> {
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(world, &request)
        .refused_because("the old run stopped after reusable artifacts were made")?;
    world.nothing_fails();
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    stored.initial_provisioning = None;
    metadata::update(&paths, &stored).required_because("remove the intent")?;
    Ok((paths, project_of(&request)?))
}

/// intentを持たない途中の状態では、観測の最後の起動はtemplateの一覧である。そのあと
/// hostの起動を挟まずに、入力の固定とintentの記録へ進む。
const LAST_OBSERVATION_STEP: &str = "sbx template ls";

fn prepared(bench: &Bench, world: &World, project: &ProjectId) -> Result<Prepared> {
    prepare(
        &bench.location,
        &bench.config,
        Some(project),
        world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
}

fn executed(bench: &Bench, world: &World, prepared: Prepared) -> Result<RepairOutput> {
    execute(
        world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
}

fn set_mode(path: &std::path::Path, mode: u32) -> Checked {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).required()
}

#[test]
fn a_repair_whose_last_observation_cannot_finish_changes_nothing() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    world.timing_out("sbx ls");

    let error = executed(&bench, &world, plan).refused_because("an unfinished observation")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none()
    );
    Ok(())
}

#[test]
fn a_repair_whose_last_observation_is_unsafe_changes_nothing() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    world.answering(
        "sha256sum",
        0,
        "0000000000000000000000000000000000000000000000000000000000000000  -\n",
    );

    let error = executed(&bench, &world, plan).refused_because("a declared file changed")?;

    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none()
    );
    Ok(())
}

#[test]
fn a_repair_whose_inputs_cannot_be_captured_records_nothing() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    let snapshot = paths.snapshot_dir();
    world.change_before(LAST_OBSERVATION_STEP, move |_| {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&snapshot, fs::Permissions::from_mode(0o000));
    });

    let error = executed(&bench, &world, plan).refused_because("unreadable inputs")?;
    set_mode(&paths.snapshot_dir(), 0o700)?;

    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none()
    );
    Ok(())
}

#[test]
fn a_repair_that_cannot_record_its_intent_changes_nothing() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    let sbxm = paths.sbxm_dir();
    world.change_before(LAST_OBSERVATION_STEP, move |_| {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&sbxm, fs::Permissions::from_mode(0o500));
    });
    let mark = world.mark();

    let error = executed(&bench, &world, plan).refused_because("an unwritable metadata")?;
    set_mode(&paths.sbxm_dir(), 0o700)?;

    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("worktree add"))
    );
    Ok(())
}

#[test]
fn a_repair_whose_cache_became_a_symlink_stops_after_the_intent() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    let cache = paths.cache_dir();
    let elsewhere = bench.workspace_root.path().join("elsewhere");
    world.change_before(LAST_OBSERVATION_STEP, move |_| {
        let _ = fs::create_dir_all(&elsewhere);
        let _ = fs::remove_dir_all(&cache);
        let _ = std::os::unix::fs::symlink(&elsewhere, &cache);
    });

    let error = executed(&bench, &world, plan).refused_because("a symlinked cache")?;

    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some()
    );
    Ok(())
}

#[test]
fn a_repair_whose_completion_cannot_be_observed_keeps_the_intent() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    world.after_the_build(|world| world.timing_out("sbx ls"));

    let error = executed(&bench, &world, plan).refused_because("an unfinished completion")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some()
    );
    Ok(())
}

#[test]
fn a_repair_whose_completion_is_unsafe_keeps_the_intent() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    world.after_the_build(|world| {
        world.answering(
            "sha256sum",
            0,
            "0000000000000000000000000000000000000000000000000000000000000000  -\n",
        );
    });

    let error = executed(&bench, &world, plan).refused_because("an unsafe completion")?;

    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some()
    );
    Ok(())
}

#[test]
fn a_repair_that_cannot_clear_its_intent_reports_the_write() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    let sbxm = paths.sbxm_dir();
    world.after_the_build(move |world| world.seal_before_the_final_record(sbxm.clone()));

    let error = executed(&bench, &world, plan).refused_because("an unwritable metadata")?;
    set_mode(&paths.sbxm_dir(), 0o700)?;

    assert_eq!(error.first_id(), Some(ErrorId::AtomicWriteFailed));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some()
    );
    Ok(())
}

#[test]
fn repair_does_not_take_a_lock_it_cannot_trust() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    set_mode(&paths.lock_file(), 0o644)?;

    let error = prepared(&bench, &world, &project).refused_because("an unsafe lock file")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    Ok(())
}

#[test]
fn repair_leaves_a_pending_rebuild_to_rebuild() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    stored.rebuild = Some(crate::metadata::RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &stored).required()?;

    let error = prepared(&bench, &world, &project).refused_because("a rebuild is pending")?;

    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    Ok(())
}

#[test]
fn repair_plans_nothing_from_an_unfinished_first_observation() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    world.timing_out("sbx ls");

    let error = prepared(&bench, &world, &project).refused_because("an unfinished observation")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    Ok(())
}

#[test]
fn repair_plans_nothing_from_an_unfinished_second_observation() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    world.change_before("sbx ls", |world| {
        world.change_before("sbx ls", |world| world.timing_out("sbx ls"));
    });

    let error = prepared(&bench, &world, &project).refused_because("an unfinished observation")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    Ok(())
}

#[test]
fn repair_plans_nothing_when_the_second_observation_is_unsafe() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    world.change_before("sbx ls", |world| {
        world.change_before("sbx ls", |world| {
            world.answering(
                "sha256sum",
                0,
                "0000000000000000000000000000000000000000000000000000000000000000  -\n",
            );
        });
    });

    let error = prepared(&bench, &world, &project).refused_because("an unsafe observation")?;

    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
    Ok(())
}

#[test]
fn repair_plans_nothing_when_docker_is_unreachable() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    world.failing("docker version");

    let error = prepared(&bench, &world, &project).refused_because("an unreachable docker")?;

    assert_eq!(error.first_id(), Some(ErrorId::DockerUnreachable));
    Ok(())
}

#[test]
fn repair_plans_nothing_when_neither_generation_has_its_image() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    fs::write(paths.dockerfile(), b"FROM example:edited\n").required()?;
    world.images.borrow_mut().clear();

    let error = prepared(&bench, &world, &project).refused_because("no generation to select")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningGenerationMissing)
    );
    Ok(())
}

#[test]
fn repair_plans_nothing_when_the_generation_is_lost_under_the_lease() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (paths, project) = legacy_incomplete(&bench, &world)?;
    fs::write(paths.dockerfile(), b"FROM example:edited\n").required()?;
    world.change_before("sbx ls", |world| {
        world.change_before("sbx ls", |world| world.images.borrow_mut().clear());
    });

    let error = prepared(&bench, &world, &project).refused_because("no generation to select")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningGenerationMissing)
    );
    Ok(())
}

/// 完成した案件の宣言fileを変えたあと、repairが計画に示す観測。
fn observed_after(change: impl Fn(&World)) -> Checked<String> {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;
    change(&world);
    let plan = prepared(&bench, &world, &project_of(&request)?).required()?;
    Ok(format!("{:?}", plan.plan.observations))
}

const DECLARED_FILE: &str = "/home/agent/.config/example/settings.yaml";

#[test]
fn repair_shows_a_declared_file_edited_inside_the_sandbox_as_modified() -> Checked {
    let observed = observed_after(|world| world.edited_inside(DECLARED_FILE, b"edited\n"))?;

    assert!(observed.contains("\"modified\""), "{observed}");
    Ok(())
}

#[test]
fn repair_shows_a_declared_file_removed_inside_the_sandbox_as_missing() -> Checked {
    let observed = observed_after(|world| {
        world.present.borrow_mut().remove(DECLARED_FILE);
    })?;

    assert!(observed.contains("\"missing\""), "{observed}");
    Ok(())
}

#[test]
fn a_repair_whose_completion_lost_a_worktree_keeps_the_intent() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    let plan = prepared(&bench, &world, &project).required()?;
    let worktree =
        SandboxLayout::new(bench.stored("Example-Org/Example-Repo")?.canonical_id()).worktree(0);
    world.after_the_build(move |world| {
        let worktree = worktree.clone();
        world.change_before("sbx ls", move |world| {
            world.present.borrow_mut().remove(&worktree);
            world.worktrees.borrow_mut().remove(&worktree);
        });
    });

    let error = executed(&bench, &world, plan).refused_because("an incomplete completion")?;

    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some()
    );
    Ok(())
}

#[test]
fn a_repair_stops_at_any_step_that_does_not_answer() -> Checked {
    let repaired = |host: &dyn HostEnvironment, bench: &Bench, project: &ProjectId| {
        let plan = prepare(
            &bench.location,
            &bench.config,
            Some(project),
            host,
            bench.workspace_root.path(),
            &mut ScriptedPrompt::choosing(0),
        )?;
        execute(
            host,
            plan,
            &bench.config,
            bench.workspace_root.path(),
            &mut SilentProgress,
        )
    };
    let bench = Bench::new()?;
    let world = World::new();
    let (_, project) = legacy_incomplete(&bench, &world)?;
    let recorded = crate::testing::host::FailingAt::recording(world);
    repaired(&recorded, &bench, &project).required_because("every step answers")?;
    for (at, step) in recorded.calls().iter().enumerate() {
        let bench = Bench::new()?;
        let world = World::new();
        let (_, project) = legacy_incomplete(&bench, &world)?;
        let failing = crate::testing::host::FailingAt::timing_out(world, at);
        assert!(repaired(&failing, &bench, &project).is_err(), "{step}");
    }
    Ok(())
}
