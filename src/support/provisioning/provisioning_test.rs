use super::*;

use crate::commands::prepare::fake::{Bench, World};
use crate::metadata;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::{image, select};
use crate::testing::add_request::request;
use crate::testing::metadata::git_identity;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::Fixture;
use crate::testing::value::DIGEST;
use crate::{design::SilentProgress, hash::sha256_hex};
use std::fs;

#[test]
fn every_provisioning_artifact_has_a_stable_display_name() {
    assert_eq!(Artifact::Sandbox.as_str(), "Sandbox");
    assert_eq!(Artifact::Workspace.as_str(), "workspace directory");
    assert_eq!(
        Artifact::Archive("/tmp/example.tar".into()).as_str(),
        "archive /tmp/example.tar"
    );
}

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
    persist_intent(&world, &mut locked, DIGEST).required_because("persist the first intent")?;
    persist_intent(&world, &mut locked, DIGEST).required_because("reuse the same intent")?;

    // 最初のtargetのimageはまだ無いため、Dockerfileを直した通常のprepareは
    // 従来どおりretargetできる。
    let retargeted = "f".repeat(64);
    persist_intent(&world, &mut locked, &retargeted)
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

    persist_intent(&world, &mut locked, DIGEST).required_because("persist the first intent")?;
    let name = locked.metadata.sandbox_name();
    let image_name = image::image_name(&name, DIGEST);
    world.images.borrow_mut().insert(
        image_name,
        image::expected_labels(locked.metadata.canonical_id(), DIGEST),
    );

    let error = persist_intent(&world, &mut locked, &"f".repeat(64))
        .refused_because("an image already built for the intent cannot be abandoned silently")?;
    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::InitialProvisioningInvalid)
    );
    Ok(())
}

#[test]
fn a_repair_preflight_requires_the_workspace_when_a_sandbox_exists() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("Example-Org/Example-Repo")?;
    let locked = locked_fixture(&fixture)?;
    let target = locked.metadata.provisioning.dockerfile_sha256.clone();

    let Err(error) = preflight(
        &locked,
        &fixture.config,
        &target,
        &World::new(),
        &fixture.workspace_root,
        true,
        true,
    ) else {
        return Err(crate::testing::outcome::Unmet::new(
            "a sandbox without its workspace is incomplete",
        ));
    };
    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::InitialProvisioningIncomplete)
    );
    Ok(())
}

#[test]
fn a_repair_preflight_skips_sandbox_checks_when_none_exists() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("Example-Org/Example-Repo")?;
    let locked = locked_fixture(&fixture)?;
    let target = locked.metadata.provisioning.dockerfile_sha256.clone();

    preflight(
        &locked,
        &fixture.config,
        &target,
        &World::new(),
        &fixture.workspace_root,
        false,
        true,
    )
    .required_because("no sandbox means nothing more to verify")?;
    Ok(())
}

#[test]
fn a_repair_preflight_passes_for_a_fully_provisioned_project() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("build a complete project")?;

    let project = crate::testing::add_request::project_of(&request)?;
    let locked = select::find(&bench.location, &project)
        .required_because("find the built project")?
        .lock()
        .required_because("lock the built project")?;
    let target = locked.metadata.provisioning.dockerfile_sha256.clone();

    preflight(
        &locked,
        &bench.config,
        &target,
        &world,
        bench.workspace_root.path(),
        true,
        true,
    )
    .required_because("a fully built project has nothing left to fix")?;
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
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("edit the Dockerfile")?;

    let name = metadata.sandbox_name();
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    let (target, warnings) = fresh_target(&world, &paths, &mut metadata, &name)?;
    assert_eq!(target, stored, "the built generation finishes the build");
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        metadata.provisioning.dockerfile_sha256, stored,
        "the stored generation is not moved while its image exists"
    );

    // imageが無くなれば、Dockerfileを直した通常のprepareは現在の世代へ移る。
    world.images.borrow_mut().clear();
    let (target, warnings) = fresh_target(&world, &paths, &mut metadata, &name)?;
    let edited = sha256_hex(b"FROM example:edited\n");
    assert_eq!(target, edited);
    assert!(warnings.is_empty());
    assert_eq!(
        metadata::load(&paths)
            .required_because("read the metadata")?
            .required_because("the project is present")?
            .provisioning
            .dockerfile_sha256,
        edited,
        "the new generation is recorded before anything is built"
    );
    Ok(())
}

#[test]
fn observation_reports_both_persistent_and_temporary_archives() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("register the project")?;
    let metadata = bench.stored("Example-Org/Example-Repo")?;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::create_dir_all(paths.cache_dir()).required_because("create the archive cache")?;
    let short = crate::hash::short_hex(&metadata.provisioning.dockerfile_sha256);
    fs::write(paths.template_archive(short), b"archive")
        .required_because("leave a persistent archive")?;
    fs::write(paths.template_archive_temp(short), b"temporary")
        .required_because("leave a temporary archive")?;

    let name = metadata.sandbox_name();
    let observation = observe(
        &world,
        &paths,
        &name,
        &metadata,
        &SandboxLayout::new(metadata.canonical_id()),
        bench.workspace_root.path(),
        true,
    )?;
    assert_eq!(observation.artifacts.len(), 2);
    assert!(
        observation
            .artifacts
            .iter()
            .all(|artifact| matches!(artifact, Artifact::Archive(_)))
    );
    Ok(())
}

#[test]
fn a_pending_project_is_reported_without_inspecting_its_artifacts() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an intent behind without a sandbox")?;
    world.nothing_fails();

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    assert!(metadata.initial_provisioning.is_some());
    let name = metadata.sandbox_name();
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());

    let observation = observe(
        &world,
        &paths,
        &name,
        &metadata,
        &SandboxLayout::new(metadata.canonical_id()),
        bench.workspace_root.path(),
        false,
    )?;

    assert_eq!(observation.state, ProvisioningState::Pending);
    assert_eq!(observation.artifacts, vec![Artifact::Sandbox]);
    assert!(observation.output.is_none());
    Ok(())
}

#[test]
fn a_fully_built_project_observes_as_ready() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first prepare succeeds")?;

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    let name = metadata.sandbox_name();
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());

    let observation = observe(
        &world,
        &paths,
        &name,
        &metadata,
        &SandboxLayout::new(metadata.canonical_id()),
        bench.workspace_root.path(),
        true,
    )?;

    assert_eq!(observation.state, ProvisioningState::Ready);
    assert!(observation.output.is_some());
    assert!(observation.artifacts.is_empty());
    Ok(())
}

#[test]
fn an_interrupted_build_with_a_running_sandbox_reports_it_alongside_the_workspace() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the run stops after the sandbox and workspace exist")?;
    world.nothing_fails();

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    assert!(
        metadata.initial_provisioning.is_some(),
        "the interrupted build still carries its intent"
    );
    let name = metadata.sandbox_name();
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());

    let observation = observe(
        &world,
        &paths,
        &name,
        &metadata,
        &SandboxLayout::new(metadata.canonical_id()),
        bench.workspace_root.path(),
        true,
    )?;

    assert_eq!(observation.state, ProvisioningState::Pending);
    assert!(observation.artifacts.contains(&Artifact::Sandbox));
    assert!(observation.artifacts.contains(&Artifact::Workspace));
    Ok(())
}

#[test]
fn matching_immutable_cache_is_not_reported_as_incomplete() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("create the reusable cache")?;

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    let sandbox = metadata.sandbox_name();
    let workspace = bench.workspace_root.path().join(sandbox.as_str());
    std::fs::remove_dir_all(&workspace).required_because("remove the old workspace")?;
    world.sandboxes.borrow_mut().clear();

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let observation = observe(
        &world,
        &paths,
        &sandbox,
        &metadata,
        &SandboxLayout::new(metadata.canonical_id()),
        bench.workspace_root.path(),
        true,
    )?;

    assert_eq!(observation.state, ProvisioningState::Fresh);
    assert!(observation.artifacts.is_empty());
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
