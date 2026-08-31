//! 初回構築intentを境にした`prepare`の外部作用。

use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;
use crate::paths;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::run;

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
