//! Dockerfileを編集した時点で、どの世代を完成させるか。
use crate::paths::ProjectPaths;
use crate::project::SandboxName;
use crate::support::image;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{
    super::fake::{Bench, World},
    *,
};
use crate::design::SilentProgress;
use crate::hash::sha256_hex;
use crate::testing::add_request::{project_of, request};
use std::fs;

/// 編集後のDockerfileの内容。世代が変わったことだけが要る。
const EDITED_DOCKERFILE: &[u8] = b"FROM example:edited\n";

#[test]
fn a_dockerfile_edited_after_the_image_exists_finishes_on_the_generation_it_started_from() -> Checked
{
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    // imageまで組み上がり、Sandboxの作成で中断した実行を作る。
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("the run stops at sandbox creation")?;
    world.nothing_fails();

    let started_from = bench
        .stored("Example-Org/Example-Repo")?
        .provisioning
        .dockerfile_sha256;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), EDITED_DOCKERFILE).required_because("edit the Dockerfile")?;

    let mark = world.mark();
    let output = run(
        &bench.location,
        &bench.config,
        &project_of(&request)?,
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("the interrupted run finishes")?;

    assert_eq!(
        output
            .warnings
            .iter()
            .map(|warning| warning.description.id)
            .collect::<Vec<_>>(),
        vec!["warning-dockerfile-changed-during-build"]
    );
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .provisioning
            .dockerfile_sha256,
        started_from,
        "the generation the build started from is the one it is finished on"
    );
    let edited = image::image_name(
        &SandboxName::derive(request.repository.canonical_id()),
        &sha256_hex(EDITED_DOCKERFILE),
    );
    assert!(
        !world.since(mark).iter().any(|call| call.contains(&edited)),
        "the edited Dockerfile is left for rebuild: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_dockerfile_edited_before_any_image_exists_is_the_generation_that_gets_built() -> Checked {
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

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), EDITED_DOCKERFILE).required_because("edit the Dockerfile")?;
    let edited = sha256_hex(EDITED_DOCKERFILE);

    let output = run(
        &bench.location,
        &bench.config,
        &project_of(&request)?,
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("the build runs on the Dockerfile that is there")?;

    assert!(output.warnings.is_empty(), "{:?}", output.warnings);
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .provisioning
            .dockerfile_sha256,
        edited,
        "the edited Dockerfile becomes the generation to build"
    );
    assert!(
        world.ran(&image::image_name(
            &SandboxName::derive(request.repository.canonical_id()),
            &edited
        )),
        "{:?}",
        world.invocations()
    );
    Ok(())
}
