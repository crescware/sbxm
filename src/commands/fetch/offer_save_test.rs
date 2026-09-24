use std::ops::ControlFlow;

use crate::commands::Context;
use crate::design::{RenderingPolicy, Ui};
use crate::diagnostics::{Diagnostic, Error, ErrorId, ExitCode};
use crate::i18n::Locale;
use crate::msg;
use crate::project::ProjectId;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

/// `offer_save`が返した流れと、書いたstdoutとstderr。
struct Offered {
    flow: ControlFlow<ExitCode>,
    stdout: String,
    stderr: String,
}

/// 動いているSandboxを1件持つ案件。
fn built() -> Checked<(Bench, World, ProjectId)> {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench.build(&world, &request).required()?;
    let project = project_of(&request)?;
    Ok((bench, world, project))
}

fn unreachable_commit() -> Diagnostic {
    Diagnostic::new(
        ErrorId::OriginCommitUnreachable,
        msg!("error-origin-commit-unreachable"),
    )
}

fn offer(
    bench: &Bench,
    world: &World,
    project: &ProjectId,
    error: &Error,
    can_prompt: bool,
    prompt: &mut ScriptedPrompt,
) -> Checked<Offered> {
    let policy = RenderingPolicy::plain();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let flow = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let context = Context {
            location: &bench.location,
            workspace_root: bench.workspace_root.path(),
            locale: Locale::En,
            can_prompt,
        };
        super::offer_save(error, project, &context, world, prompt, &mut ui)
    };
    Ok(Offered {
        flow,
        stdout: String::from_utf8(stdout).required_because("offer stdout is UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("offer stderr is UTF-8")?,
    })
}

#[test]
fn choosing_to_save_fetches_and_lets_the_caller_prepare_again() -> Checked {
    let (bench, world, project) = built()?;

    let offered = offer(
        &bench,
        &world,
        &project,
        &Error::single(unreachable_commit()),
        true,
        &mut ScriptedPrompt::choosing(0),
    )?;

    assert_eq!(offered.flow, ControlFlow::Continue(()));
    // 何を失うのかを見せてから訊き、`sbxm fetch`と同じ結果を示す。
    assert!(
        offered.stderr.contains("not reachable"),
        "{}",
        offered.stderr
    );
    assert!(
        offered.stdout.contains("no branch or tag"),
        "{}",
        offered.stdout
    );
    assert!(world.ran("bundle create"), "{:?}", world.invocations());
    Ok(())
}

#[test]
fn choosing_to_stop_reports_the_refusal_once_and_saves_nothing() -> Checked {
    let (bench, world, project) = built()?;
    let mark = world.mark();

    let offered = offer(
        &bench,
        &world,
        &project,
        &Error::single(unreachable_commit()),
        true,
        &mut ScriptedPrompt::choosing(1),
    )?;

    assert_eq!(offered.flow, ControlFlow::Break(ExitCode::Failure));
    assert_eq!(
        offered.stderr.matches("not reachable").count(),
        1,
        "{}",
        offered.stderr
    );
    assert!(world.since(mark).is_empty(), "{:?}", world.since(mark));
    Ok(())
}

#[test]
fn canceling_the_question_is_reported_as_a_cancel() -> Checked {
    let (bench, world, project) = built()?;

    let offered = offer(
        &bench,
        &world,
        &project,
        &Error::single(unreachable_commit()),
        true,
        &mut ScriptedPrompt::canceling(),
    )?;

    assert_eq!(offered.flow, ControlFlow::Break(ExitCode::Canceled));
    Ok(())
}

#[test]
fn a_choice_outside_the_offer_is_not_taken_as_consent() -> Checked {
    let (bench, world, project) = built()?;
    let mark = world.mark();

    let offered = offer(
        &bench,
        &world,
        &project,
        &Error::single(unreachable_commit()),
        true,
        &mut ScriptedPrompt::choosing(2),
    )?;

    assert_eq!(offered.flow, ControlFlow::Break(ExitCode::Failure));
    assert!(world.since(mark).is_empty(), "{:?}", world.since(mark));
    Ok(())
}

#[test]
fn a_refusal_that_saving_cannot_resolve_is_reported_without_asking() -> Checked {
    // 未commitの変更はhostへ保存しても残らない。訊けばcancelになるpromptで、訊かない
    // ことを確かめる。
    let (bench, world, project) = built()?;
    let error = Error::Diagnostics(vec![
        unreachable_commit(),
        Diagnostic::new(
            ErrorId::WorktreeTrackedChanges,
            msg!("error-worktree-tracked-changes"),
        ),
    ]);

    let offered = offer(
        &bench,
        &world,
        &project,
        &error,
        true,
        &mut ScriptedPrompt::canceling(),
    )?;

    assert_eq!(offered.flow, ControlFlow::Break(ExitCode::Failure));
    assert!(!world.ran("bundle create"), "{:?}", world.invocations());
    Ok(())
}

#[test]
fn a_cancel_is_reported_without_asking() -> Checked {
    let (bench, world, project) = built()?;

    let offered = offer(
        &bench,
        &world,
        &project,
        &Error::Canceled,
        true,
        &mut ScriptedPrompt::choosing(0),
    )?;

    assert_eq!(offered.flow, ControlFlow::Break(ExitCode::Canceled));
    assert!(!world.ran("bundle create"), "{:?}", world.invocations());
    Ok(())
}

#[test]
fn without_a_terminal_the_refusal_is_reported_without_asking() -> Checked {
    let (bench, world, project) = built()?;

    let offered = offer(
        &bench,
        &world,
        &project,
        &Error::single(unreachable_commit()),
        false,
        &mut ScriptedPrompt::choosing(0),
    )?;

    assert_eq!(offered.flow, ControlFlow::Break(ExitCode::Failure));
    assert!(!world.ran("bundle create"), "{:?}", world.invocations());
    Ok(())
}

#[test]
fn a_save_that_fails_is_reported_and_does_not_continue() -> Checked {
    let (bench, world, project) = built()?;
    world.stopped();

    let offered = offer(
        &bench,
        &world,
        &project,
        &Error::single(unreachable_commit()),
        true,
        &mut ScriptedPrompt::choosing(0),
    )?;

    assert_eq!(offered.flow, ControlFlow::Break(ExitCode::Failure));
    assert!(
        offered.stderr.contains("sandbox-not-running"),
        "{}",
        offered.stderr
    );
    Ok(())
}
