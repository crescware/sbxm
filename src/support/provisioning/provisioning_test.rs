use super::*;

use crate::design::SilentProgress;
use crate::support::select;
use crate::testing::add_request::request;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::provisioning::{Bench, World};
use std::fs;

#[test]
fn provisioning_states_keep_their_stable_spellings() {
    for (state, expected) in [
        (ProvisioningState::Fresh, "fresh"),
        (ProvisioningState::Ready, "ready"),
        (ProvisioningState::Pending, "pending"),
        (ProvisioningState::Incomplete, "incomplete"),
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

    let mark = world.mark();
    let output = provision(
        &mut locked,
        &bench.config,
        &generation,
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
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("docker image save") || call.contains("sbx create")),
        "verified artifacts are reused: {:?}",
        world.since(mark)
    );
    Ok(())
}
