use crate::diagnostics::ErrorId;

use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};
use crate::testing::scripted_clock::ScriptedClock;

use super::*;

#[test]
fn a_sandbox_without_anything_to_save_is_reported_as_such() -> Checked {
    let clock = ScriptedClock::default();
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
        &clock,
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
    let clock = ScriptedClock::default();
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
        &clock,
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

/// 構築済みの案件へ、`change`を加えてから保存する。
fn saved_after(
    change: impl Fn(&Bench, &crate::paths::ProjectPaths) -> Checked,
) -> Checked<crate::diagnostics::Result<SaveOutput>> {
    let clock = ScriptedClock::default();
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;
    let paths =
        crate::paths::ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    change(&bench, &paths)?;
    world.answering(crate::support::host_sync::PLACE_SAVE_REFS, 0, "ready\n");
    world.timing_out("fetch --no-tags");
    Ok(save_now(
        &bench.location,
        Some(&project_of(&request)?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
        &clock,
    ))
}

#[test]
fn a_save_does_not_take_a_lock_it_cannot_trust() -> Checked {
    let error = saved_after(|_, paths| {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(paths.lock_file(), std::fs::Permissions::from_mode(0o644))
            .required()
    })?
    .err()
    .required_because("an unsafe lock")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    Ok(())
}

#[test]
fn a_save_during_a_rebuild_is_refused() -> Checked {
    let error = saved_after(|bench, paths| {
        let mut stored = bench.stored("Example-Org/Example-Repo")?;
        stored.rebuild = Some(crate::metadata::RebuildIntent {
            target_dockerfile_sha256: "2".repeat(64),
            previous_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
        });
        crate::metadata::update(paths, &stored).required()
    })?
    .err()
    .required_because("a rebuild is pending")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    Ok(())
}

#[test]
fn a_save_that_does_not_arrive_is_reported() -> Checked {
    let error = saved_after(|_, _| Ok(()))?
        .err()
        .required_because("the host does not receive")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    Ok(())
}
