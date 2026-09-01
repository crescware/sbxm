//! 初回構築intentを境にした`prepare`の外部作用。

use std::fs;

use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;
use crate::paths::{self, ProjectPaths};
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::run;

/// projectを登録するだけで、まだ構築しない。
fn registered(bench: &Bench, request: &crate::commands::add::AddRequest, world: &World) -> Checked {
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        request,
        &crate::testing::metadata::git_identity(),
        world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;
    Ok(())
}

#[test]
fn an_interrupted_prepare_keeps_the_target_and_file_inputs() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");

    let error = bench
        .build(&world, &request)
        .refused_because("the first provisioning mutation can fail")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let intent = stored
        .initial_provisioning
        .required_because("the first mutation leaves a repair intent")?;
    assert_eq!(
        intent.target_dockerfile_sha256, stored.provisioning.dockerfile_sha256,
        "the target is stored together with the provisioning generation"
    );
    assert_eq!(intent.files.len(), 1);
    assert_eq!(
        intent.files[0].source,
        paths::display(bench.config.files[0].source.as_path())
    );
    assert_eq!(
        intent.files[0].destination,
        paths::display(bench.config.files[0].destination.as_path())
    );
    assert_eq!(
        intent.files[0].sha256,
        sha256_hex(b"declared = true\n"),
        "the declared file is pinned by content digest"
    );
    Ok(())
}

#[test]
fn prepare_observes_an_intent_without_resuming_it() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("docker build");
    bench
        .build(&world, &request)
        .refused_because("the initial image build is interrupted")?;
    world.nothing_fails();

    let mark = world.mark();
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("prepare requires explicit repair after interruption")?;

    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert!(
        !world.since(mark).iter().any(|call| {
            call.contains("docker build")
                || call.contains("sbx create")
                || call.contains("template load")
        }),
        "prepare only observes the pending state: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_dockerfile_edited_after_the_snapshot_does_not_change_what_is_built() -> Checked {
    // intentを保存した瞬間から、build工程はsnapshotだけを読む。生きているDockerfileを
    // build直前に書き換えても、既にfixした世代へは影響しない。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    registered(&bench, &request, &world)?;
    let project = project_of(&request)?;
    let paths = ProjectPaths::derive(&bench.parent, &project.canonical());
    let dockerfile = paths.dockerfile();
    let original =
        fs::read(&dockerfile).required_because("read the original Dockerfile before editing it")?;

    let edited = dockerfile.clone();
    world.mutate_before("docker build", move || {
        let _ = fs::write(&edited, b"FROM example:edited-during-the-race\n");
    });

    run(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .required_because("prepare still succeeds because it never reads the live Dockerfile again")?;

    let build = world
        .calls
        .borrow()
        .iter()
        .find(|spec| {
            spec.program == "docker" && spec.args.first().map(String::as_str) == Some("build")
        })
        .cloned()
        .required_because("a build ran")?;
    let file_index = build
        .args
        .iter()
        .position(|arg| arg == "--file")
        .required_because("the build names a dockerfile")?;
    assert_ne!(
        build.args[file_index + 1],
        paths::display(&dockerfile),
        "the build reads a fixed snapshot, not the live path that raced it"
    );

    let stored = bench.stored("Example-Org/Example-Repo")?;
    assert_eq!(
        stored.provisioning.dockerfile_sha256,
        sha256_hex(&original),
        "generation A's label is not attached to content B: the digest matches what was \
         actually snapshotted before the race, not the edit"
    );
    Ok(())
}

#[test]
fn a_declared_file_edited_after_the_snapshot_does_not_change_what_is_copied() -> Checked {
    // 同じ理由で、宣言fileも`sbx cp`直前の書き換えに影響されない。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    registered(&bench, &request, &world)?;
    let project = project_of(&request)?;

    let source = bench.config.files[0].source.as_path().to_path_buf();
    let original =
        fs::read(&source).required_because("read the original declared file before editing it")?;

    let edited = source.clone();
    world.mutate_before("cp --follow-link", move || {
        let _ = fs::write(&edited, b"changed = true # raced the copy\n");
    });

    run(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .required_because("prepare still succeeds because it never reads the live source again")?;

    let placed = world
        .digests
        .borrow()
        .get("/home/agent/.config/example/settings.yaml")
        .cloned()
        .required_because("the placed file's digest is observable in the sandbox")?;
    assert_eq!(
        placed,
        sha256_hex(&original),
        "the byte stream that was copied matches the intent's digest, not the edit that raced it"
    );
    Ok(())
}
