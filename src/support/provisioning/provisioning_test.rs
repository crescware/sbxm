use super::*;

use crate::commands::prepare::fake::{Bench, World};
use crate::metadata;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::select;
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
        Artifact::Image("example-image".into()).as_str(),
        "image example-image"
    );
    assert_eq!(
        Artifact::Template("example-template".into()).as_str(),
        "Template example-template"
    );
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
fn intent_persistence_is_idempotent_and_rejects_a_different_target() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("Example-Org/Example-Repo")?;
    let mut locked = locked_fixture(&fixture)?;

    clear_intent(&mut locked).required_because("clearing an absent intent is a no-op")?;
    persist_intent(&mut locked, DIGEST).required_because("persist the first intent")?;
    persist_intent(&mut locked, DIGEST).required_because("reuse the same intent")?;

    let error = persist_intent(&mut locked, &"f".repeat(64))
        .refused_because("a different target cannot reuse the intent")?;
    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::InitialProvisioningInvalid)
    );

    clear_intent(&mut locked).required_because("clear the completed intent")?;
    assert!(locked.metadata.initial_provisioning.is_none());
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
    let (target, warnings) = fresh_target(&world, &paths, &name, &metadata)?;
    assert_eq!(target, metadata.provisioning.dockerfile_sha256);
    assert_eq!(warnings.len(), 1);

    world.images.borrow_mut().clear();
    let (target, warnings) = fresh_target(&world, &paths, &name, &metadata)?;
    assert_eq!(target, sha256_hex(b"FROM example:edited\n"));
    assert!(warnings.is_empty());
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
    let workspace = bench
        .workspace_root
        .path()
        .join(locked.metadata.sandbox_name().as_str());
    fs::remove_dir_all(&workspace).required_because("remove the neutral workspace")?;

    let mark = world.mark();
    let output = provision(
        &mut locked,
        &bench.config,
        &target,
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
