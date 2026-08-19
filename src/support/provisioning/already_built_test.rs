use std::fs;

use crate::paths::ProjectPaths;
use crate::project::{SandboxLayout, SandboxName};

use crate::testing::outcome::{Checked, Required};

use crate::commands::prepare::fake::{Bench, World};

use super::already_built;
use crate::testing::add_request::{project_of, request};

#[test]
fn a_running_sandboxs_vanished_workspace_is_not_folded_into_a_no_op_success() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let add_request = request("Example-Org/Example-Repo", None, None)?;
    let built = bench
        .build(&world, &add_request)
        .required_because("the first run builds everything")?;
    assert!(!built.already_built);

    let canonical = project_of(&add_request)?.canonical();
    let paths = ProjectPaths::derive(&bench.parent, &canonical);
    let name = SandboxName::derive(&canonical);
    let layout = SandboxLayout::new(&canonical);
    let metadata = bench.stored(&add_request.repository.display_id())?;

    // Sandboxの中はworldの状態が持つため、runningのままlive mountが答え続ける。
    // mount元のdirectoryだけがhostから消えた状態を作る。
    let workspace = bench.workspace_root.path().join(&built.sandbox);
    fs::remove_dir_all(&workspace).required_because("the workspace directory is removed")?;

    let output = already_built(
        &world,
        &paths,
        &name,
        &metadata,
        &layout,
        bench.workspace_root.path(),
    )
    .required_because("observing the host does not itself fail")?;

    assert!(
        output.is_none(),
        "a workspace that is gone on host must not be folded into a silent no-op success"
    );
    Ok(())
}
