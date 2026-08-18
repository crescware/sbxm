use crate::cli::Interactivity;
use crate::commands::Context;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{OutputPolicy, PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::paths::ProjectPaths;
use crate::project::ProjectId;

use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use std::fs;
use std::os::unix::fs::PermissionsExt;

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
fn a_successful_prepare_prints_each_nonfatal_warning() -> Checked {
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
    .required_because("register the project")?;

    let paths = ProjectPaths::derive(&bench.parent, add_request.repository.canonical_id());
    fs::create_dir_all(paths.cache_dir()).required_because("create the archive cache")?;
    fs::set_permissions(paths.cache_dir(), fs::Permissions::from_mode(0o300))
        .required_because("make the cache writable but not listable")?;

    let outcome = run_exec(&bench, &world, Some(&project_of(&add_request)?));

    fs::set_permissions(paths.cache_dir(), fs::Permissions::from_mode(0o700))
        .required_because("restore the cache permissions")?;
    let (code, _, printed_err) = outcome?;
    assert_eq!(code, ExitCode::Success);
    assert!(
        printed_err.contains("could not be inspected"),
        "the warning reaches stderr: {printed_err}"
    );
    Ok(())
}
