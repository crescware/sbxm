use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::metadata;
use crate::paths::{self, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};
use std::cell::Cell;
use std::fs;
use std::path::PathBuf;

use super::{execute, prepare};

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
fn an_interrupted_prepare_keeps_its_intent_until_explicit_repair() -> Checked {
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
    let error = crate::commands::prepare::run::run(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("the failed first mutation leaves an intent")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some()
    );
    world.nothing_fails();

    let error = crate::commands::prepare::run::run(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("prepare does not implicitly resume the intent")?;
    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));

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
fn repair_refuses_changed_global_file_input_before_observing_the_host() -> Checked {
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

    let mark = world.mark();
    let error = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("repair does not use changed intent input")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningInputChanged)
    );
    assert!(
        world.since(mark).is_empty(),
        "input validation precedes host observation: {:?}",
        world.since(mark)
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

    crate::commands::prepare::run::run(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
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
    let generation = stored.provisioning.dockerfile_sha256.clone();
    stored.initial_provisioning = Some(
        crate::support::provisioning::initial_intent(&bench.config, &generation)
            .required_because("record the already-complete intent")?,
    );
    metadata::update(&paths, &stored).required_because("persist the already-complete intent")?;

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
    assert!(
        !world.since(mark).iter().any(|call| {
            call.contains("docker build") || call.contains("sbx create") || call.contains("sbx cp")
        }),
        "a complete pending project is only observed and cleared: {:?}",
        world.since(mark)
    );
    Ok(())
}
