use crate::commands::add::AddRequest;
use crate::metadata;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;
use crate::support::host_sync::AutoSaved;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use crate::testing::provisioning::{Bench, World};

use super::save_first;

fn local_request() -> Checked<AddRequest> {
    Ok(AddRequest {
        repository: RepositoryIdentity::local("/home/user/code/app/.git", "app")
            .required_because("a local repository")?,
        worktrees: None,
        detach: None,
        start_branch: Some("main".to_string()),
        parent_inside_repository: false,
    })
}

fn local_project() -> Checked<ProjectId> {
    ProjectId::parse("local/app").required()
}

fn tried_to_save(world: &World, mark: usize) -> bool {
    world
        .since(mark)
        .iter()
        .any(|call| call.contains(crate::support::host_sync::PLACE_SAVE_REFS))
}

#[test]
fn a_running_local_sandbox_is_asked_for_its_commits() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    let mark = world.mark();

    // 模したSandboxはbundleを書かない。保存するものが無いSandboxとして答える。
    let saved = save_first(
        &bench.location,
        &local_project()?,
        &world,
        bench.workspace_root.path(),
        &mut crate::design::SilentProgress,
    );

    assert!(matches!(saved, AutoSaved::Nothing), "{saved:?}");
    assert!(tried_to_save(&world, mark), "{:?}", world.since(mark));
    Ok(())
}

#[test]
fn nothing_is_saved_where_there_is_nothing_to_save_from() -> Checked {
    // GitHubの案件、停止中のSandbox、管理していない案件、世代の切替の途中の案件。
    let bench = Bench::new()?;
    let world = World::new();
    let github = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &github).required()?;
    bench.build(&world, &local_request()?).required()?;
    let mark = world.mark();

    for project in [
        project_of(&github)?,
        ProjectId::parse("local/other").required()?,
    ] {
        let saved = save_first(
            &bench.location,
            &project,
            &world,
            bench.workspace_root.path(),
            &mut crate::design::SilentProgress,
        );
        assert!(matches!(saved, AutoSaved::Nothing), "{project}: {saved:?}");
    }

    let mut stored = bench.stored("local/app")?;
    let paths = crate::paths::ProjectPaths::derive(&bench.parent, stored.canonical_id());
    stored.rebuild = Some(crate::metadata::RebuildIntent {
        target_dockerfile_sha256: crate::testing::metadata::OTHER_DIGEST.to_string(),
        previous_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &stored).required()?;
    let saved = save_first(
        &bench.location,
        &local_project()?,
        &world,
        bench.workspace_root.path(),
        &mut crate::design::SilentProgress,
    );
    assert!(matches!(saved, AutoSaved::Nothing), "{saved:?}");
    assert!(!tried_to_save(&world, mark), "{:?}", world.since(mark));
    Ok(())
}

#[test]
fn a_stopped_local_sandbox_is_not_started_to_save_from() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    world.stopped();
    let mark = world.mark();

    let saved = save_first(
        &bench.location,
        &local_project()?,
        &world,
        bench.workspace_root.path(),
        &mut crate::design::SilentProgress,
    );

    assert!(matches!(saved, AutoSaved::Nothing), "{saved:?}");
    assert!(!tried_to_save(&world, mark), "{:?}", world.since(mark));
    Ok(())
}

#[test]
fn a_lock_that_cannot_be_taken_is_a_warning_rather_than_silence() -> Checked {
    // 保存できなかったことを黙らない。続く操作が同じ理由で断るとは限らない。
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    let stored = bench.stored("local/app")?;
    let paths = crate::paths::ProjectPaths::derive(&bench.parent, stored.canonical_id());
    std::fs::remove_file(paths.lock_file()).required()?;
    std::os::unix::fs::symlink("/dev/null", paths.lock_file()).required()?;

    let saved = save_first(
        &bench.location,
        &local_project()?,
        &world,
        bench.workspace_root.path(),
        &mut crate::design::SilentProgress,
    );

    let AutoSaved::Failed(warning) = saved else {
        return Err(crate::testing::outcome::Unmet::new(format!("{saved:?}")));
    };
    assert_eq!(warning.description.id, "auto-save-failed");
    Ok(())
}

#[test]
fn a_save_that_must_not_wait_gives_up_at_once_on_a_held_lock() -> Checked {
    // sessionのあいだの保存は、lockを待つあいだSSHの終了に気付けない。取れなければ
    // すぐに諦め、次の機会に保存する。
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    let candidate = crate::support::select::find(&bench.location, &local_project()?).required()?;
    let held = candidate.paths.acquire_lock().required()?;

    let started = std::time::Instant::now();
    let saved = super::save_selected(
        candidate,
        &world,
        bench.workspace_root.path(),
        std::time::Duration::ZERO,
        &mut crate::design::SilentProgress,
    );

    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(saved, AutoSaved::Failed(_)), "{saved:?}");
    drop(held);
    Ok(())
}
