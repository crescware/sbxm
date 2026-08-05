//! Sandboxに入っているtoolに応じた設定。

use crate::testing::outcome::{Checked, Required};

use super::super::fake::{Bench, World};
use crate::testing::add_request::request;

#[test]
fn a_worktree_that_declares_mise_is_never_looked_for() -> Checked {
    // 中で何を動かすかは案件の話であり、sbxmは持たない。宣言fileを探しにも行かない。
    let bench = Bench::new()?;
    let world = World::new();
    world.carrying("/home/agent/work/example-repo/example-repo.tree-0/mise.toml");

    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required_because("build")?;
    assert!(
        !world.ran("mise.toml"),
        "the declared files are not even looked for: {:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn a_sandbox_without_gh_is_never_asked_to_configure_it() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    world.without("gh");

    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required_because("build")?;
    assert!(!world.ran("git_protocol"), "{:?}", world.invocations());
    Ok(())
}
