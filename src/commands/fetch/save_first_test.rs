use crate::commands::add::AddRequest;
use crate::metadata;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;
use crate::support::bundle::AutoSaved;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use crate::testing::provisioning::{Bench, World};

use super::save_first;

fn local_request() -> Checked<AddRequest> {
    Ok(AddRequest {
        repository: RepositoryIdentity::local("/home/user/code/app", "app")
            .required_because("a local repository")?,
        worktrees: None,
        detach: None,
        start_branch: Some("main".to_string()),
    })
}

fn local_project() -> Checked<ProjectId> {
    ProjectId::parse("local/app").required()
}

fn tried_to_save(world: &World, mark: usize) -> bool {
    world
        .since(mark)
        .iter()
        .any(|call| call.contains("bundle create"))
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
    );

    assert!(matches!(saved, AutoSaved::Nothing), "{saved:?}");
    assert!(!tried_to_save(&world, mark), "{:?}", world.since(mark));
    Ok(())
}
