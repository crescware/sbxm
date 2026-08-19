use super::*;

use crate::commands::prepare::fake::{Bench, World};
use crate::metadata;
use crate::paths::ProjectPaths;
use crate::project::ProjectId;
use crate::support::{image, select};
use crate::testing::add_request::request;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::Fixture;
use crate::testing::value::DIGEST;
use crate::{design::SilentProgress, hash::sha256_hex};
use std::fs;

fn locked_fixture(fixture: &Fixture) -> Checked<crate::support::select::Locked> {
    let project = ProjectId::parse("Example-Org/Example-Repo").required()?;
    select::find(&fixture.location, &project)
        .required_because("find the registered project")?
        .lock()
        .required_because("lock the registered project")
}

#[test]
fn intent_persistence_is_idempotent_and_retargets_before_anything_is_built() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("Example-Org/Example-Repo")?;
    let mut locked = locked_fixture(&fixture)?;
    let world = World::new();

    clear_intent(&mut locked).required_because("clearing an absent intent is a no-op")?;
    persist_intent(&world, &mut locked, DIGEST, None)
        .required_because("persist the first intent")?;
    persist_intent(&world, &mut locked, DIGEST, None).required_because("reuse the same intent")?;

    // 最初のtargetのimageはまだ無いため、Dockerfileを直した通常のprepareは
    // 従来どおりretargetできる。
    let retargeted = "f".repeat(64);
    persist_intent(&world, &mut locked, &retargeted, None)
        .required_because("no artifact was built for the abandoned target yet")?;
    assert_eq!(
        locked
            .metadata
            .initial_provisioning
            .as_ref()
            .map(|intent| intent.target_dockerfile_sha256.clone()),
        Some(retargeted.clone())
    );
    assert_eq!(locked.metadata.provisioning.dockerfile_sha256, retargeted);

    clear_intent(&mut locked).required_because("clear the completed intent")?;
    assert!(locked.metadata.initial_provisioning.is_none());
    Ok(())
}

#[test]
fn a_different_target_is_rejected_once_the_abandoned_targets_image_is_built() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("Example-Org/Example-Repo")?;
    let mut locked = locked_fixture(&fixture)?;
    let world = World::new();

    persist_intent(&world, &mut locked, DIGEST, None)
        .required_because("persist the first intent")?;
    let name = locked.metadata.sandbox_name();
    let image_name = image::image_name(&name, DIGEST);
    world.images.borrow_mut().insert(
        image_name,
        image::expected_labels(locked.metadata.canonical_id(), DIGEST),
    );

    let error = persist_intent(&world, &mut locked, &"f".repeat(64), None)
        .refused_because("an image already built for the intent cannot be abandoned silently")?;
    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::InitialProvisioningInvalid)
    );
    Ok(())
}

#[test]
fn fresh_target_keeps_a_built_generation_when_the_dockerfile_changes() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an image from the interrupted build")?;
    world.nothing_fails();

    let mut metadata = bench.stored("Example-Org/Example-Repo")?;
    metadata.initial_provisioning = None;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    metadata::update(&paths, &metadata).required_because("remove the intent")?;
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("edit the Dockerfile")?;

    let name = metadata.sandbox_name();
    let fresh = fresh_target(&world, &paths, &name, &metadata)?;
    assert_eq!(fresh.generation, metadata.provisioning.dockerfile_sha256);
    assert_eq!(fresh.warnings.len(), 1);

    world.images.borrow_mut().clear();
    let fresh = fresh_target(&world, &paths, &name, &metadata)?;
    assert_eq!(fresh.generation, sha256_hex(b"FROM example:edited\n"));
    assert!(fresh.warnings.is_empty());
    Ok(())
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
    let target = locked
        .metadata
        .initial_provisioning
        .as_ref()
        .map(|intent| intent.target_dockerfile_sha256.clone())
        .required_because("the interrupted run recorded its target")?;
    let target = TargetSelection {
        generation: target,
        warnings: Vec::new(),
        stored: None,
    };
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
        target,
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
    assert!(locked.metadata.initial_provisioning.is_none());
    Ok(())
}
