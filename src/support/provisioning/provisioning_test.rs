use super::*;

use crate::design::SilentProgress;
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::support::select;
use crate::testing::add_request::request;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::poll::poll;
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::Duration;

#[test]
fn provisioning_states_keep_their_stable_spellings() {
    for (state, expected) in [
        (ProvisioningState::Fresh, "fresh"),
        (ProvisioningState::Ready, "ready"),
        (ProvisioningState::Pending, "pending"),
        (ProvisioningState::Incomplete, "incomplete"),
        (ProvisioningState::Unobservable, "unobservable"),
    ] {
        assert_eq!(state.as_str(), expected);
        assert_eq!(state.to_string(), expected);
    }
}

#[test]
fn provisioning_reuses_verified_artifacts_and_reports_a_restored_workspace() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("interrupt after creating the reusable artifacts")?;
    world.nothing_fails();

    let project = crate::testing::add_request::project_of(&request)?;
    let mut locked = select::find(&bench.location, &project)
        .required_because("find the interrupted project")?
        .lock()
        .required_because("lock the interrupted project")?;
    let generation = locked.metadata.provisioning.dockerfile_sha256.clone();
    let workspace = bench
        .workspace_root
        .path()
        .join(locked.metadata.sandbox_name().as_str());
    fs::remove_dir_all(&workspace).required_because("remove the neutral workspace")?;

    let name = locked.metadata.sandbox_name();
    let preconditions = verify_external_preconditions(&world, &name)
        .required_because("secret and docker preconditions are met")?;
    let inputs = ProvisioningInputs::capture(&locked.paths, &bench.config, Some(&generation))
        .required_because("capture the snapshot that fixes this attempt")?;

    let mark = world.mark();
    let output = provision(
        &mut locked,
        &inputs,
        preconditions,
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
        Vec::new(),
    )
    .required_because("resume through the shared provisioning boundary")?;

    assert!(workspace.is_dir(), "{}", workspace.display());
    assert!(
        output
            .warnings
            .iter()
            .any(|warning| warning.description.id == "warning-workspace-restored")
    );
    // Templateのruntime idは、label検証済みのimageから作るarchiveのconfig digestと
    // 照合してから再利用可否を決める。そのため`docker image save`自体は毎回起こるが、
    // 検証を通った場合は`sbx template load`や`sbx create`のような再構築へは進まない。
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("template load") || call.contains("sbx create")),
        "verified artifacts are reused: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn construction_holds_exclusive_then_connection_holds_shared() -> Checked {
    // project lock → exclusive → sharedの順序は、`open`が準備mutationと接続を1回で
    // 済ませるようになったあとも変わらない。構築中は他のsession/lifecycle操作を
    // 入れず、接続中はexclusiveな操作（rebuild/destroyなど）だけを締め出す。
    let bench = Bench::new()?;
    let world = World::new();
    let add_request = request("Example-Org/Example-Repo", None, None)?;
    let project = bench
        .register(&world, &add_request)
        .required_because("the project is registered")?;
    let paths = ProjectPaths::derive(&bench.parent, &project.canonical());
    let lease_file = paths.session_lease_file();

    let refused_during_construction = Rc::new(RefCell::new(None));
    let recorded = Rc::clone(&refused_during_construction);
    world.mutate_before("sbx create", move || {
        let refused = paths::acquire_shared_lock(
            &lease_file,
            Duration::from_millis(50),
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
        .is_err();
        *recorded.borrow_mut() = Some(refused);
    });

    let prepared = crate::commands::open::run::prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        None,
        &world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
        poll(),
        &mut SilentProgress,
    )
    .required_because("the first open builds and connects")?;

    assert_eq!(
        refused_during_construction.take(),
        Some(true),
        "a shared lock attempt made while the exclusive session lease is held is refused"
    );

    // 準備が終わり接続まで進んだあとは、shared session leaseがexclusiveな操作を
    // 締め出す（sharedな他sessionとは共存できる、と混同しない）。
    paths::acquire_exclusive_lock(
        &paths.session_lease_file(),
        Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("an active connection blocks a new exclusive session lease")?;

    drop(prepared);
    paths::acquire_exclusive_lock(
        &paths.session_lease_file(),
        Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the session lease releases once the session ends")?;
    Ok(())
}
