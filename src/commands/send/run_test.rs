use crate::commands::add::AddRequest;
use crate::diagnostics::ErrorId;
use crate::project::ProjectId;
use crate::repository::RepositoryIdentity;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::*;

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

#[test]
fn the_host_branches_are_sent_and_fetched_into_the_sandbox_origin() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    let mark = world.mark();

    let output = run(
        &bench.location,
        Some(&local_project()?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .required()?;

    assert_eq!(output.project, "local/app");
    assert_eq!(
        output.repository,
        std::path::Path::new("/home/user/code/app")
    );
    let calls = world.since(mark);
    let position = |needle: &str| {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .required_because(&format!("no command matched {needle}: {calls:?}"))
    };
    assert!(
        position("bundle create --quiet")? < position("fetch --prune origin")?,
        "the bundle reaches the sandbox before it is fetched"
    );
    // worktreeのbranchは動かさない。取り込むかどうかはSandboxの中で決める。
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("merge") || call.contains("reset")),
        "{calls:?}"
    );
    Ok(())
}

#[test]
fn a_github_project_is_not_sent_to() -> Checked {
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
    .refused_because("its sandbox fetches from GitHub itself")?;

    assert_eq!(error.first_id(), Some(ErrorId::SendRequiresLocal));
    assert!(world.since(mark).is_empty(), "{:?}", world.since(mark));
    Ok(())
}

#[test]
fn a_stopped_sandbox_is_not_started_to_send_to() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench.build(&world, &local_request()?).required()?;
    world.stopped();
    let mark = world.mark();

    let error = run(
        &bench.location,
        Some(&local_project()?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .refused_because("a stopped sandbox is refused")?;

    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    assert!(
        !world.since(mark).iter().any(|call| call.contains("bundle")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_sandbox_whose_repository_is_not_set_up_receives_nothing() -> Checked {
    // bundleを置く場所は、この案件のbare repositoryの中である。repositoryになる前の
    // 場所へ置けば、続きの構築がそこを既存のrepositoryと読み違える。
    let bench = Bench::new()?;
    let world = World::new();
    world.failing("remote add origin");
    bench
        .build(&world, &local_request()?)
        .refused_because("the build stopped before the origin was set")?;
    world.nothing_fails();
    let mark = world.mark();

    run(
        &bench.location,
        Some(&local_project()?),
        &mut ScriptedPrompt::choosing(0),
        &world,
        bench.workspace_root.path(),
    )
    .refused_because("the repository is not ready")?;

    assert!(
        !world.since(mark).iter().any(|call| call.contains("bundle")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn sending_from_the_entry_point_prints_what_changed() -> Checked {
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
        super::super::exec(
            Some(&local_project()?),
            &context,
            &mut ui,
            &world,
            &mut prompt,
        )
    };
    assert_eq!(code, ExitCode::Success);
    let stdout = String::from_utf8(stdout).required()?;
    assert!(stdout.contains("already has every branch"), "{stdout}");
    Ok(())
}
