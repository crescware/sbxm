//! 進捗の見せ方と、宣言fileの配置結果。
use super::super::world::{World, bench};
use crate::command::{OutputPolicy, TimeoutClass};
use crate::hash::sha256_hex;
use crate::support::files::Placement;
use crate::testing::add_request::request;

#[test]
fn the_long_steps_forward_their_progress_and_the_read_steps_are_captured() {
    let bench = bench();
    let world = World::new();
    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");

    for (needle, timeout) in [
        ("docker build", TimeoutClass::ImageBuild),
        ("docker image save", TimeoutClass::ImageBuild),
        ("git clone git@github.com", TimeoutClass::RepositoryTransfer),
        ("sbx template load", TimeoutClass::SandboxLifecycle),
        ("sbx create", TimeoutClass::SandboxLifecycle),
        ("fetch --prune origin", TimeoutClass::RepositoryTransfer),
    ] {
        assert_eq!(
            world.policy_of(needle),
            Some((OutputPolicy::Passthrough, timeout)),
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
}

#[test]
fn the_declared_file_is_placed_once_and_left_alone_afterwards() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("build");

    assert_eq!(
        world
            .digests
            .borrow()
            .get("/home/agent/.config/example/config.toml")
            .map(String::as_str),
        Some(sha256_hex(b"declared = true\n").as_str()),
        "the declared file reaches the destination it was declared for"
    );
    assert!(
        !world.present.borrow().contains("/tmp/sbxm-file-0"),
        "the staged copy does not survive the placement"
    );

    // 同じ内容の再配置は、Sandboxへ書き込まない。
    let world = World::new();
    world.digests.borrow_mut().insert(
        "/home/agent/.config/example/config.toml".to_string(),
        sha256_hex(b"declared = true\n"),
    );
    world
        .present
        .borrow_mut()
        .insert("/home/agent/.config/example/config.toml".to_string());
    let output = bench.build(&world, &request).expect("build");
    assert_eq!(output.files[0].placement, Placement::Unchanged);
    assert!(
        !world.ran("sbx cp"),
        "an identical destination is left alone"
    );
}
