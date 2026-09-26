use crate::commands::add::AddRequest;
use crate::diagnostics::ErrorId;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;
use crate::support::host_sync::PLACE_SAVE_REFS;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::*;

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

fn sync(bench: &Bench, world: &World) -> crate::diagnostics::Result<SyncOutput> {
    let project = ProjectId::parse("local/app")?;
    run(
        &bench.location,
        Some(&project),
        &mut ScriptedPrompt::choosing(0),
        world,
        bench.workspace_root.path(),
    )
}

#[test]
fn the_sandbox_is_saved_and_reflected_before_the_host_is_sent_back() -> Checked {
    // Sandboxのoriginは、同期したあとのhostを映す。
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    world.answering(PLACE_SAVE_REFS, 0, "ready\n");
    let mark = world.mark();

    let output = sync(&bench, &world).required()?;

    assert_eq!(output.project, "local/app");
    assert_eq!(output.reflected, Some(Vec::new()));
    assert!(
        output.namespace.starts_with("sbxm-local-app-"),
        "{output:?}"
    );
    let calls = world.since(mark);
    let position = |needle: &str| {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .required_because(&format!("no command matched {needle}: {calls:?}"))
    };
    let saved = position(&format!(
        "fetch --no-tags --no-recurse-submodules --no-write-fetch-head --quiet ssh://{}.sbx",
        output.namespace
    ))?;
    let reflected = position("push --porcelain --no-verify .")?;
    let sent = position(".git +refs/heads/*:refs/remotes/origin/*")?;
    assert!(position(PLACE_SAVE_REFS)? < saved, "{calls:?}");
    assert!(saved < reflected && reflected < sent, "{calls:?}");
    Ok(())
}

#[test]
fn a_sandbox_with_nothing_to_save_is_still_sent_the_host() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    let mark = world.mark();

    let output = sync(&bench, &world).required()?;

    assert_eq!(output.reflected, None);
    let calls = world.since(mark);
    // 保存するものが無ければ、hostのbranchへの反映も無い。
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("push --porcelain --no-verify . ")),
        "{calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|call| call.contains(".git +refs/heads/*:refs/remotes/origin/*")),
        "{calls:?}"
    );
    Ok(())
}

#[test]
fn a_github_project_is_not_synced() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;
    let mark = world.mark();

    let error = run(
        &bench.location,
        Some(&project_of(&request)?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .refused_because("its sandbox fetches from and pushes to GitHub itself")?;

    assert_eq!(error.first_id(), Some(ErrorId::SyncRequiresLocal));
    assert!(world.since(mark).is_empty(), "{:?}", world.since(mark));
    Ok(())
}

#[test]
fn an_omitted_project_is_chosen_only_from_the_projects_added_with_local() -> Checked {
    // GitHubの案件しか無ければ、選ばせる前に断る。
    let bench = Bench::new()?;
    let world = World::new();
    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required()?;
    let mut prompt = ScriptedPrompt::choosing(0);
    let mark = world.mark();

    let error = run(
        &bench.location,
        None,
        &mut prompt,
        &world,
        bench.workspace_root.path(),
    )
    .refused_because("there is no local project to sync")?;

    assert_eq!(error.first_id(), Some(ErrorId::NoLocalProjects));
    assert!(prompt.asked.borrow().is_empty(), "no prompt is shown");
    assert!(world.since(mark).is_empty(), "{:?}", world.since(mark));

    // localの案件を足せば、それだけが並ぶ。
    bench.build(&world, &local_request()?).required()?;
    let mut prompt = ScriptedPrompt::choosing(0);
    let output = run(
        &bench.location,
        None,
        &mut prompt,
        &world,
        bench.workspace_root.path(),
    )
    .required_because("the local project is synced")?;

    assert_eq!(output.project, "local/app");
    assert_eq!(prompt.asked.borrow()[0], vec!["local/app".to_string()]);
    Ok(())
}

#[test]
fn a_stopped_sandbox_is_not_started_to_sync_with() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    world.stopped();
    let mark = world.mark();

    let error = sync(&bench, &world).refused_because("a stopped sandbox is refused")?;

    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains(PLACE_SAVE_REFS) || call.contains("ssh://")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_sandbox_whose_repository_is_not_set_up_is_neither_read_nor_sent_to() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    world.failing("remote add origin");
    bench
        .build(&world, &local_request()?)
        .refused_because("the build stopped before the origin was set")?;
    world.nothing_fails();
    let mark = world.mark();

    sync(&bench, &world).refused_because("the repository is not ready")?;

    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains(PLACE_SAVE_REFS) || call.contains("ssh://")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn syncing_from_the_entry_point_prints_what_changed() -> Checked {
    use crate::commands::Context;
    use crate::design::prompt::{RecordedScreen, ScriptedKeys};
    use crate::design::{PromptUi, RenderingPolicy, Ui};
    use crate::diagnostics::ExitCode;
    use crate::i18n::Locale;

    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;

    let mut stdout: Vec<u8> = Vec::new();
    let policy = RenderingPolicy::plain();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, std::io::sink());
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(ScriptedKeys::confirming()),
            Box::new(RecordedScreen::new()),
        );
        let context = Context {
            location: &bench.location,
            workspace_root: bench.workspace_root.path(),
            locale: Locale::En,
            can_prompt: false,
        };
        crate::commands::sync::exec(
            Some(&local_project()?),
            &context,
            &mut ui,
            &world,
            &mut prompt,
        )
    };

    assert_eq!(code, ExitCode::Success);
    let text = String::from_utf8(stdout).required()?;
    assert!(text.contains("local/app"), "{text}");
    Ok(())
}
