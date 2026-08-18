use crate::cli::Interactivity;
use crate::commands::Context;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{OutputPolicy, PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::project::ProjectId;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};

use std::fs;

use crate::commands::prepare::fake::{Bench, World};

fn run_exec(
    bench: &Bench,
    world: &World,
    project: Option<&ProjectId>,
) -> Checked<(ExitCode, String, String)> {
    let context = Context {
        location: &bench.location,
        workspace_root: bench.workspace_root.path(),
        lang: Some(Locale::En),
        interactivity: Interactivity {
            stdin_is_tty: false,
            stderr_is_tty: false,
        },
    };
    let policy = OutputPolicy::plain();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(ScriptedKeys::choosing(0)),
            Box::new(RecordedScreen::new()),
        );
        super::exec(project, &context, &mut ui, world, &mut prompt)
    };
    let printed = String::from_utf8(stdout).required_because("stdout is valid UTF-8")?;
    let printed_err = String::from_utf8(stderr).required_because("stderr is valid UTF-8")?;
    Ok((code, printed, printed_err))
}

#[test]
fn a_fresh_project_is_reported_as_a_no_op_success() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let add_request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &add_request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut crate::design::SilentProgress,
    )
    .required_because("register the fresh project")?;

    let project = project_of(&add_request)?;
    let (code, printed, _) = run_exec(&bench, &world, Some(&project))?;

    assert_eq!(code, ExitCode::Success);
    assert!(printed.contains("does not need repair"), "{printed}");
    Ok(())
}

#[test]
fn an_execution_error_is_reported_as_failure() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let add_request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &add_request)
        .refused_because("leave an interrupted project")?;
    world.failing("sbx create");

    let project = project_of(&add_request)?;
    let (code, _, printed_err) = run_exec(&bench, &world, Some(&project))?;

    assert_eq!(code, ExitCode::Failure);
    assert!(!printed_err.is_empty(), "the failed repair is reported");
    Ok(())
}

#[test]
fn a_successful_repair_prints_the_result_and_each_warning() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let add_request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &add_request)
        .refused_because("leave an interrupted project")?;
    world.nothing_fails();

    let sandbox = world.sandboxes.borrow()[0].name.clone();
    let workspace = bench.workspace_root.path().join(&sandbox);
    fs::remove_dir_all(&workspace).required_because("remove the neutral workspace")?;

    let project = project_of(&add_request)?;
    let (code, printed, printed_err) = run_exec(&bench, &world, Some(&project))?;

    assert_eq!(code, ExitCode::Success);
    assert!(printed.contains("was repaired"), "{printed}");
    assert!(
        printed_err.contains(&sandbox),
        "the warning names the restored workspace: {printed_err}"
    );
    Ok(())
}
