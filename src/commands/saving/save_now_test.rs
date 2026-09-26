use crate::diagnostics::ErrorId;

use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::*;

#[test]
fn a_sandbox_without_anything_to_save_is_reported_as_such() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;

    // 模したSandboxは、保存するrefが無いSandboxとして答える。
    let output = save_now(
        &bench.location,
        Some(&project_of(&request)?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .required()?;
    assert_eq!(output.changes, None);
    assert_eq!(output.project, "Example-Org/Example-Repo");
    assert!(
        world.ran(crate::support::host_sync::PLACE_SAVE_REFS),
        "the sandbox is asked for its commits: {:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn a_stopped_sandbox_is_not_started_to_save_from() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;
    world.stopped();
    let mark = world.mark();

    let error = save_now(
        &bench.location,
        Some(&project_of(&request)?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .err()
    .required_because("a stopped sandbox is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    assert!(
        !world.since(mark).iter().any(|call| call.contains("exec")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}
