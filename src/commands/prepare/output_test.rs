//! 進捗の見せ方と、宣言fileの配置結果。

use crate::testing::outcome::{Checked, Required};

use super::super::fake::{Bench, World};
use crate::command::{OutputPolicy, TimeoutClass};
use crate::hash::sha256_hex;
use crate::testing::add_request::request;

#[test]
fn the_long_steps_forward_their_progress_and_the_read_steps_are_captured() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required_because("build")?;

    for (needle, timeout) in [
        ("docker build", TimeoutClass::ImageBuild),
        ("docker image save", TimeoutClass::ImageBuild),
        (
            "git clone --progress git@github.com",
            TimeoutClass::RepositoryTransfer,
        ),
        ("sbx template load", TimeoutClass::SandboxLifecycle),
        ("sbx create", TimeoutClass::SandboxLifecycle),
        (
            "fetch --prune --progress origin",
            TimeoutClass::RepositoryTransfer,
        ),
    ] {
        assert_eq!(
            world.policy_of(needle),
            Some((OutputPolicy::Relay, timeout)),
            "{needle} shows its progress while it runs"
        );
    }

    for needle in [
        "sbx ls --json",
        "docker image inspect",
        "sbx secret ls",
        "sbx template ls --json",
    ] {
        assert_eq!(
            world.policy_of(needle).map(|(output, _)| output),
            Some(OutputPolicy::Capture),
            "{needle} is read rather than shown"
        );
    }
    Ok(())
}

#[test]
fn the_declared_file_is_placed_and_its_staging_copy_is_removed() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    let output = bench
        .build(&world, &request)
        .required_because("the initial provisioning succeeds")?;

    assert_eq!(
        world
            .digests
            .borrow()
            .get("/home/agent/.config/example/settings.yaml")
            .map(String::as_str),
        Some(sha256_hex(b"declared = true\n").as_str()),
        "the declared file reaches the destination it was declared for"
    );
    assert!(
        !world.present.borrow().contains("/tmp/sbxm-file-0"),
        "the staged copy does not survive the placement"
    );
    assert_eq!(output.files.len(), 1);
    Ok(())
}
