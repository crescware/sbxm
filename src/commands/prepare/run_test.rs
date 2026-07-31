use crate::testing::outcome::{Checked, Refused, Required};

use super::super::world::{World, bench};
use super::*;
use crate::error::ErrorId;
use crate::hash::sha256_hex;
use crate::testing::add_request::{project_of, request};
use crate::testing::project::project_id;
use crate::ui::SilentProgress;
use std::fs;

#[test]
fn a_project_that_is_not_registered_is_sent_to_add() -> Checked {
    let bench = bench()?;
    let world = World::new();

    let error = run(
        &bench.location,
        &bench.config,
        &project_id("example-org/example-repo")?,
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .refused_because("there is nothing to build yet")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-project-not-managed")
    );
    assert!(
        world.invocations().is_empty(),
        "nothing is asked of the host: {:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn an_unregistered_project_gets_no_lock_file() -> Checked {
    let bench = bench()?;
    let world = World::new();
    let project = project_id("example-org/example-repo")?;
    let paths = ProjectPaths::derive(&bench.parent, &project.canonical());
    // lock fileを置ける状態、つまりmetadataのない`.sbxm`だけがある状態で確かめる。
    fs::create_dir_all(paths.sbxm_dir())
        .required_because("the project directory is left behind")?;

    run(
        &bench.location,
        &bench.config,
        &project,
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .refused_because("there is nothing to build yet")?;

    assert!(
        !paths.lock_file().exists(),
        "an unregistered project is not given a lock file"
    );
    assert_eq!(
        fs::read_dir(paths.sbxm_dir())
            .required_because("read the project directory")?
            .count(),
        0,
        "nothing is written under an unregistered project"
    );
    Ok(())
}

#[test]
fn a_rebuild_in_progress_builds_nothing() -> Checked {
    let bench = bench()?;
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
    let mut stored = metadata::load(&paths)
        .required_because("read the metadata")?
        .required_because("present")?;
    stored.rebuild = Some(metadata::RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"target"),
        previous_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &stored).required_because("record the intent")?;

    let mark = world.mark();
    let error = run(
        &bench.location,
        &bench.config,
        &project_of(&request)?,
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .refused_because("a half-switched project is not built on")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));

    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to get out of it")?;
    assert_eq!(remediation.explanation[0].id, "remediation-run-rebuild");
    // 実行するcommandは説明文ではなく、独立した一行として持つ。
    let command = remediation
        .commands
        .first()
        .required_because("the remediation carries the command to run")?;
    assert_eq!(command.as_str(), "sbxm rebuild Example-Org/Example-Repo");

    assert!(
        world.since(mark).is_empty(),
        "nothing is asked of the host: {:?}",
        world.since(mark)
    );
    Ok(())
}
