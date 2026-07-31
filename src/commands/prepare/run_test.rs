use super::super::world::{World, bench};
use super::*;
use crate::error::ErrorId;
use crate::hash::sha256_hex;
use crate::testing::add_request::{project_of, request};
use crate::testing::project::project_id;
use crate::ui::SilentProgress;
use std::fs;

#[test]
fn a_project_that_is_not_registered_is_sent_to_add() {
    let bench = bench();
    let world = World::new();

    let error = run(
        &bench.config,
        &project_id("example-org/example-repo"),
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .expect_err("there is nothing to build yet");
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
}

#[test]
fn an_unregistered_project_gets_no_lock_file() {
    let bench = bench();
    let world = World::new();
    let project = project_id("example-org/example-repo");
    let paths = ProjectPaths::derive(&bench.config.base_path, &project.canonical());
    // lock fileを置ける状態、つまりmetadataのない`.sbxm`だけがある状態で確かめる。
    fs::create_dir_all(paths.sbxm_dir()).expect("the project directory is left behind");

    run(
        &bench.config,
        &project,
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .expect_err("there is nothing to build yet");

    assert!(
        !paths.lock_file().exists(),
        "an unregistered project is not given a lock file"
    );
    assert_eq!(
        fs::read_dir(paths.sbxm_dir())
            .expect("read the project directory")
            .count(),
        0,
        "nothing is written under an unregistered project"
    );
}

#[test]
fn a_rebuild_in_progress_builds_nothing() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    crate::commands::add::run::run(&bench.config, &request, &world, &mut SilentProgress)
        .expect("the project is registered");

    let paths = ProjectPaths::derive(&bench.config.base_path, request.repository.canonical_id());
    let mut stored = metadata::load(&paths)
        .expect("read the metadata")
        .expect("present");
    stored.rebuild = Some(metadata::RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"target"),
        previous_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &stored).expect("record the intent");

    let mark = world.mark();
    let error = run(
        &bench.config,
        &project_of(&request),
        &world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .expect_err("a half-switched project is not built on");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));

    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .expect("the user is told how to get out of it");
    assert_eq!(remediation.explanation[0].id, "remediation-run-rebuild");
    // 実行するcommandは説明文ではなく、独立した一行として持つ。
    let command = remediation
        .commands
        .first()
        .expect("the remediation carries the command to run");
    assert_eq!(command.as_str(), "sbxm rebuild Example-Org/Example-Repo");

    assert!(
        world.since(mark).is_empty(),
        "nothing is asked of the host: {:?}",
        world.since(mark)
    );
}
