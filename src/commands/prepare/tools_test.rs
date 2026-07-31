//! Sandboxに入っているtoolが返した案内。

use crate::testing::outcome::{Checked, Required};

use super::super::fake::{Bench, World};
use crate::testing::add_request::request;

/// managed worktreeが持ち込んだ`mise.toml`のpath。
const DECLARED_MISE: &str = "/home/agent/work/example-repo/example-repo.tree-0/mise.toml";

#[test]
fn what_the_tools_answer_reaches_the_output() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    world.carrying(DECLARED_MISE);

    let output = bench
        .build(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required_because("build")?;
    assert_eq!(output.notes.len(), 1, "{:?}", output.notes);
    assert_eq!(output.notes[0].items, vec![DECLARED_MISE.to_string()]);
    Ok(())
}

#[test]
fn a_tool_the_sandbox_lacks_never_reaches_the_output() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    world.without("mise");
    world.carrying(DECLARED_MISE);

    let output = bench
        .build(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required_because("build")?;
    assert!(output.notes.is_empty(), "{:?}", output.notes);
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
