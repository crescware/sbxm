use crate::diagnostics::ErrorId;
use crate::support::Observed;
use crate::support::select;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::provisioning::{Bench, World};

use super::{ProvisioningState, observe};

/// 構築済み案件を、lockを取らずにもう一度観測する。
fn observe_built(bench: &Bench, world: &World, project: &crate::project::ProjectId) -> Checked<()> {
    let candidate = select::find(&bench.location, project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;
    let observation = observe(
        world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("observe the built project")?;
    assert_eq!(observation.state, ProvisioningState::Ready);
    Ok(())
}

#[test]
fn a_completed_project_is_observed_as_ready() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    observe_built(&bench, &world, &project_of(&request)?)
}

#[test]
fn a_stopped_sandbox_is_observed_as_unobservable_rather_than_incomplete() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    // 完成した案件のSandboxが止まっただけで、成果物は何も失われていない。
    world.stopped();
    let project = project_of(&request)?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;

    let mark = world.mark();
    let observation = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("observing a stopped project does not fail")?;

    assert_eq!(
        observation.state,
        ProvisioningState::Unobservable,
        "a stopped sandbox is not a partially built one"
    );
    observation
        .require_safe()
        .required_because("being stopped is not an unsafe observation")?;
    for observed in [
        &observation.repository,
        &observation.worktrees_present,
        &observation.identity,
        &observation.files_placed,
    ] {
        assert_eq!(observed.as_str(), "not-observed", "{observed:?}");
    }
    // 中を読むcommandはSandboxを起動し得る。observeはそれを1つも実行しない。
    assert!(
        !world.since(mark).iter().any(|call| call.contains("exec")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn only_the_missing_worktree_among_several_ends_the_matching_state() -> Checked {
    // 1本欠けているだけで走査を打ち切ると、存在するworktreeの一覧が空のまま返り、
    // 呼び出し側（`actions_for`など）が要求本数すべてを欠落として扱ってしまう。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", Some(2), Some("main"))?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    let project = project_of(&request)?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;
    let layout = crate::project::SandboxLayout::new(metadata.canonical_id());
    let missing = layout.worktree(1);
    world.present.borrow_mut().remove(&missing);
    world.worktrees.borrow_mut().remove(&missing);

    let observation = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("one missing worktree does not end the observation")?;

    assert_eq!(observation.worktrees_present.as_str(), "missing");
    assert_eq!(
        observation.worktrees.len(),
        1,
        "the worktree that is still present is still reported: {:?}",
        observation.worktrees
    );
    assert_eq!(observation.worktrees[0].path, layout.worktree_name(0));
    Ok(())
}

#[test]
fn an_unsafe_artifact_is_recorded_without_ending_the_observation() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    // 同名のSandboxが、この案件のものではないworkspaceを指している。
    for row in world.sandboxes.borrow_mut().iter_mut() {
        row.workspace = "/tmp/somebody-elses-workspace".to_string();
    }
    let project = project_of(&request)?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;

    let observation = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("one unsafe artifact does not end the observation")?;

    assert!(
        matches!(observation.sandbox, Observed::Mismatch { .. }),
        "{:?}",
        observation.sandbox
    );
    // 観測は続き、他のartifactの事実も残る。
    assert!(observation.stored_image_present);
    let error = observation
        .require_safe()
        .refused_because("a mutation cannot start from an unsafe observation")?;
    assert!(error.contains_id(ErrorId::SandboxUnusable));
    Ok(())
}
