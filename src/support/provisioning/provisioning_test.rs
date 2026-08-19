use super::*;

use crate::commands::prepare::fake::{Bench, World};
use crate::metadata;
use crate::paths::ProjectPaths;
use crate::support::select;
use crate::testing::add_request::request;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::{design::SilentProgress, hash::sha256_hex};
use std::fs;

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
