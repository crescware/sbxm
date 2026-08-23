use crate::commands::Context;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::project::ProjectId;

use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};

use std::fs;

use super::super::fake::{Bench, World};

/// `exec`を1回起動し、終了code・stdout・stderrへ書かれた内容を返す。
fn run_exec(
    bench: &Bench,
    world: &World,
    project: Option<&ProjectId>,
) -> Checked<(ExitCode, String, String)> {
    let context = Context {
        location: &bench.location,
        workspace_root: bench.workspace_root.path(),
        locale: Locale::En,
        can_prompt: false,
    };
    let policy = RenderingPolicy::plain();
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
fn a_finished_build_is_reported_as_a_no_op_success() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let add_request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &add_request)
        .required_because("the first run builds everything")?;

    let project = project_of(&add_request)?;
    let (code, printed, _) = run_exec(&bench, &world, Some(&project))?;

    assert_eq!(code, ExitCode::Success);
    assert!(
        printed.contains("is already built"),
        "an unchanged run says so rather than claiming work: {printed}"
    );
    Ok(())
}

#[test]
fn a_workspace_that_had_to_be_created_again_is_warned_about_by_the_command_wrapper() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let add_request = request("Example-Org/Example-Repo", None, None)?;
    // worktreeを揃える工程で止め、Sandboxだけができている状態を作る。
    world.failing("worktree add");
    bench
        .build(&world, &add_request)
        .refused_because("the run stops at the step that failed")?;
    world.nothing_fails();

    // 続きを実行する前に、hostのworkspace directoryだけが消える。
    let sandbox = world.sandboxes.borrow()[0].name.clone();
    let workspace = bench.workspace_root.path().join(&sandbox);
    fs::remove_dir_all(&workspace).required_because("the workspace directory is removed")?;

    let project = project_of(&add_request)?;
    let (code, _, printed_err) = run_exec(&bench, &world, Some(&project))?;

    assert_eq!(code, ExitCode::Success);
    assert!(
        printed_err.contains(&sandbox),
        "the warning names the sandbox whose workspace was recreated: {printed_err}"
    );
    Ok(())
}
