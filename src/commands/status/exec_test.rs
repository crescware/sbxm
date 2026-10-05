use crate::commands::{Context, status::Scope};
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::project::ProjectId;
use crate::testing::global_status::FakeHost;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::Fixture;
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::scripted_clock::ScriptedClock;

use super::select_scope;

fn execute_prompt(
    fixture: &Fixture,
    host: &dyn crate::boundary::host::HostEnvironment,
    keys: ScriptedKeys,
    screen: &RecordedScreen,
) -> ExitCode {
    let clock = ScriptedClock::default();
    let policy = RenderingPolicy::plain();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(keys),
            Box::new(screen.clone()),
        );
        let context = Context {
            location: &fixture.location,
            workspace_root: &fixture.workspace_root,
            clock: &clock,
            locale: Locale::En,
            can_prompt: true,
        };
        super::exec(&Scope::Prompt, &context, &mut ui, host, &mut prompt)
    }
}

#[test]
fn global_is_the_first_choice_and_projects_follow_in_registry_order() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("zeta-org/zeta-repo")?;
    fixture.register("example-org/example-repo")?;

    let mut prompt = ScriptedPrompt::choosing(0);
    assert_eq!(select_scope(&fixture.location, &mut prompt)?, Scope::Global);
    assert_eq!(
        prompt.asked.borrow()[0],
        vec![
            "global".to_owned(),
            "example-org/example-repo".to_owned(),
            "zeta-org/zeta-repo".to_owned(),
        ]
    );

    let mut prompt = ScriptedPrompt::choosing(2);
    assert_eq!(
        select_scope(&fixture.location, &mut prompt)?,
        Scope::Project(ProjectId::parse("zeta-org/zeta-repo")?)
    );
    Ok(())
}

#[test]
fn global_is_available_even_when_no_project_is_registered() -> Checked {
    let fixture = Fixture::new()?;
    let mut prompt = ScriptedPrompt::choosing(0);

    assert_eq!(select_scope(&fixture.location, &mut prompt)?, Scope::Global);
    assert_eq!(prompt.asked.borrow()[0], vec!["global".to_owned()]);
    Ok(())
}

#[test]
fn prompt_executes_the_global_scope() -> Checked {
    let fixture = Fixture::new()?;
    let screen = RecordedScreen::new();

    let code = execute_prompt(
        &fixture,
        &FakeHost::macos(),
        ScriptedKeys::choosing(0),
        &screen,
    );

    assert_ne!(code, ExitCode::Canceled);
    assert!(screen.drawn().iter().any(|line| line.contains("global")));
    Ok(())
}

#[test]
fn prompt_executes_the_selected_project_scope() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let screen = RecordedScreen::new();

    let code = execute_prompt(
        &fixture,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        ScriptedKeys::choosing(1),
        &screen,
    );

    assert_ne!(code, ExitCode::Canceled);
    Ok(())
}

#[test]
fn project_progress_is_flushed_before_waiting_for_docker_in_both_languages() -> Checked {
    use std::io::BufWriter;

    use crate::testing::command::ScriptedWriter;
    use crate::testing::host::ChangingBefore;

    for (locale, waiting) in [
        (Locale::En, "Waiting for Docker to respond."),
        (Locale::Ja, "Dockerの応答を待っています。"),
    ] {
        for scope in [
            Scope::Prompt,
            Scope::Project(ProjectId::parse("example-org/example-repo")?),
        ] {
            let fixture = Fixture::new()?;
            fixture.register("example-org/example-repo")?;
            let clock = ScriptedClock::default();
            let policy = RenderingPolicy::plain();
            let stdout = ScriptedWriter::accepting_all();
            let stderr = ScriptedWriter::accepting_all();
            let written_stdout = stdout.written();
            let written_stderr = stderr.written();
            let visible_stderr = written_stderr.clone();
            let host = ChangingBefore::new(
                FakeSbx::listing(r#"{"sandboxes":[]}"#),
                "image ls --quiet",
                move || {
                    // BufWriterの中に残った表示では、待ち時間のfeedbackにならない。
                    let progress = String::from_utf8_lossy(&visible_stderr.borrow()).into_owned();
                    assert!(progress.contains("example-org/example-repo"), "{progress}");
                    assert!(progress.contains(waiting), "{progress}");
                    assert!(written_stdout.borrow().is_empty(), "the report comes later");
                },
            );
            let screen = RecordedScreen::new();
            let mut prompt = PromptUi::new(
                locale,
                policy.stderr,
                Box::new(ScriptedKeys::choosing(1)),
                Box::new(screen.clone()),
            );
            let context = Context {
                location: &fixture.location,
                workspace_root: &fixture.workspace_root,
                clock: &clock,
                locale,
                can_prompt: matches!(scope, Scope::Prompt),
            };
            let mut ui = Ui::new(
                locale,
                policy,
                BufWriter::new(stdout),
                BufWriter::new(stderr),
            );

            let code = super::exec(&scope, &context, &mut ui, &host, &mut prompt);

            assert_ne!(code, ExitCode::Canceled);
            assert!(host.inner.ran("image ls --quiet"));
            let progress = String::from_utf8_lossy(&written_stderr.borrow()).into_owned();
            assert!(!progress.contains('\u{1b}'), "{progress}");
            assert_eq!(screen.drawn().is_empty(), !context.can_prompt);
        }
    }
    Ok(())
}

#[test]
fn global_progress_is_flushed_before_each_external_check_in_both_languages() -> Checked {
    use std::cell::Cell;
    use std::io::BufWriter;
    use std::rc::Rc;

    use crate::testing::command::ScriptedWriter;
    use crate::testing::host::ChangingBefore;

    let fixture = Fixture::new()?;
    let clock = ScriptedClock::default();
    for (needle, skip, en, ja) in [
        (
            "version --format",
            0,
            "Checking Docker connectivity.",
            "Dockerへの疎通を確認します。",
        ),
        ("version", 1, "CLI version", "CLIのversionを確認します。"),
        (
            "policy ls",
            0,
            "network policy",
            "network policyを確認します。",
        ),
        (
            "daemon status",
            0,
            "daemon state",
            "daemonの状態を確認します。",
        ),
        ("ls --json", 0, "sign-in", "loginを確認します。"),
        (
            "-G",
            0,
            "Remote SSH configuration",
            "Remote SSHの設定を確認します。",
        ),
    ] {
        for (locale, expected) in [(Locale::En, en), (Locale::Ja, ja)] {
            let stdout = ScriptedWriter::accepting_all();
            let stderr = ScriptedWriter::accepting_all();
            let visible_stdout = stdout.written();
            let visible_stderr = stderr.written();
            let observed = Rc::new(Cell::new(false));
            let observed_by_host = observed.clone();
            let host = ChangingBefore::skipping(FakeHost::macos(), needle, skip, move || {
                let progress = String::from_utf8_lossy(&visible_stderr.borrow()).into_owned();
                assert!(progress.contains(expected), "{needle}: {progress}");
                assert!(visible_stdout.borrow().is_empty(), "the report comes later");
                observed_by_host.set(true);
            });
            let policy = RenderingPolicy::plain();
            let mut ui = Ui::new(
                locale,
                policy,
                BufWriter::new(stdout),
                BufWriter::new(stderr),
            );
            let mut prompt = PromptUi::new(
                locale,
                policy.stderr,
                Box::new(ScriptedKeys::canceling()),
                Box::new(RecordedScreen::new()),
            );
            let context = Context {
                location: &fixture.location,
                workspace_root: &fixture.workspace_root,
                clock: &clock,
                locale,
                can_prompt: false,
            };
            assert_ne!(
                super::exec(&Scope::Global, &context, &mut ui, &host, &mut prompt),
                ExitCode::Canceled
            );
            assert!(observed.get(), "the check was not reached: {needle}");
        }
    }
    Ok(())
}

#[test]
fn inside_progress_is_flushed_before_each_slow_read_without_fetching() -> Checked {
    use std::io::BufWriter;

    use crate::testing::command::ScriptedWriter;
    use crate::testing::host::ChangingBefore;
    use crate::testing::protection::clean_host;

    for (needle, en, ja) in [
        (
            "worktree list --porcelain",
            "Reading the worktree list",
            "worktree一覧を読み取ります",
        ),
        (
            "status --porcelain=v2 -z --untracked-files=all",
            "changes and untracked files",
            "変更と未追跡fileを確認します",
        ),
        (
            "rev-parse HEAD",
            "HEAD and upstream",
            "HEADとupstreamを確認します",
        ),
        (
            "config --get remote.origin.url",
            "No fetch is performed.",
            "fetchは行いません",
        ),
    ] {
        for (locale, waiting) in [(Locale::En, en), (Locale::Ja, ja)] {
            let fixture = Fixture::new()?;
            let project = fixture.register("example-org/example-repo")?;
            let clock = ScriptedClock::default();
            let policy = RenderingPolicy::plain();
            let stdout = ScriptedWriter::accepting_all();
            let stderr = ScriptedWriter::accepting_all();
            let written_stdout = stdout.written();
            let written_stderr = stderr.written();
            let visible_stderr = written_stderr.clone();
            let host = ChangingBefore::new(clean_host(&fixture, &project)?, needle, move || {
                let progress = String::from_utf8_lossy(&visible_stderr.borrow()).into_owned();
                assert!(progress.contains(waiting), "{progress}");
                if needle.starts_with("status ") || needle == "rev-parse HEAD" {
                    assert!(progress.contains("worktree 1/1"), "{progress}");
                    assert!(progress.contains("example-repo.tree-0"), "{progress}");
                }
                assert!(written_stdout.borrow().is_empty(), "the report comes later");
            });
            let mut ui = Ui::new(
                locale,
                policy,
                BufWriter::new(stdout),
                BufWriter::new(stderr),
            );
            let mut prompt = PromptUi::new(
                locale,
                policy.stderr,
                Box::new(ScriptedKeys::choosing(1)),
                Box::new(RecordedScreen::new()),
            );
            let context = Context {
                location: &fixture.location,
                workspace_root: &fixture.workspace_root,
                clock: &clock,
                locale,
                can_prompt: true,
            };

            assert_ne!(
                super::exec(&Scope::Prompt, &context, &mut ui, &host, &mut prompt),
                ExitCode::Canceled
            );
            assert!(host.inner.ran(needle), "{needle}");
            assert!(!host.inner.ran(" fetch "), "status must remain read-only");
            let progress = String::from_utf8_lossy(&written_stderr.borrow()).into_owned();
            assert!(!progress.contains('\u{1b}'), "{progress}");
        }
    }
    Ok(())
}

#[test]
fn prompt_cancellation_returns_canceled() -> Checked {
    let fixture = Fixture::new()?;
    let screen = RecordedScreen::new();

    let code = execute_prompt(
        &fixture,
        &FakeHost::macos(),
        ScriptedKeys::canceling(),
        &screen,
    );

    assert_eq!(code, ExitCode::Canceled);
    Ok(())
}

#[test]
fn explicit_global_status_reports_login_alongside_other_host_checks() -> Checked {
    let clock = ScriptedClock::default();
    let fixture = Fixture::new()?;
    let host = FakeHost::macos().failing(
        "sbx ls --json",
        "user is not authenticated to Docker: secret not found",
        1,
    );
    let screen = RecordedScreen::new();
    let policy = RenderingPolicy::plain();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(ScriptedKeys::canceling()),
            Box::new(screen.clone()),
        );
        let context = Context {
            location: &fixture.location,
            workspace_root: &fixture.workspace_root,
            clock: &clock,
            locale: Locale::En,
            can_prompt: false,
        };
        super::exec(&Scope::Global, &context, &mut ui, &host, &mut prompt)
    };
    assert_eq!(code, ExitCode::Failure);
    let stdout = String::from_utf8(stdout).required()?;
    let stderr = String::from_utf8(stderr).required()?;
    assert!(stdout.contains("Docker Sandboxes login"), "{stdout}");
    assert!(stdout.contains("Platform"), "{stdout}");
    assert!(stderr.contains("sbx-login-missing"), "{stderr}");
    assert!(stderr.contains("sbx login"), "{stderr}");
    assert!(screen.drawn().is_empty());
    Ok(())
}

/// 他人にも読めるconfig。configは読む前にpermissionを確かめる。
fn exposed_config(fixture: &Fixture) -> Checked {
    use std::os::unix::fs::PermissionsExt;
    let config = fixture.location.config_file();
    std::fs::create_dir_all(fixture.location.dir()).required()?;
    std::fs::set_permissions(
        fixture.location.dir(),
        std::fs::Permissions::from_mode(0o700),
    )
    .required()?;
    std::fs::write(&config, b"language: en\n").required()?;
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o644)).required()
}

#[test]
fn a_prompt_over_an_unreadable_config_asks_nothing() -> Checked {
    let fixture = Fixture::new()?;
    exposed_config(&fixture)?;
    let screen = RecordedScreen::new();

    let code = execute_prompt(
        &fixture,
        &FakeHost::macos(),
        ScriptedKeys::choosing(0),
        &screen,
    );

    assert_eq!(code, ExitCode::Failure);
    assert!(screen.drawn().is_empty());
    Ok(())
}

#[test]
fn a_prompt_over_an_unreadable_registry_is_reported() -> Checked {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let registry = fixture.location.registry_file();
    std::fs::set_permissions(&registry, std::fs::Permissions::from_mode(0o644)).required()?;
    let screen = RecordedScreen::new();

    let code = execute_prompt(
        &fixture,
        &FakeHost::macos(),
        ScriptedKeys::choosing(0),
        &screen,
    );

    assert_eq!(code, ExitCode::Failure);
    Ok(())
}

#[test]
fn a_choice_past_the_listed_projects_is_not_resolved() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;

    let error = select_scope(&fixture.location, &mut ScriptedPrompt::choosing(2))
        .refused_because("there is no second project")?;

    assert!(error.first_id().is_some());
    Ok(())
}

#[test]
fn a_project_status_over_an_unreadable_config_is_reported() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    exposed_config(&fixture)?;
    let clock = ScriptedClock::default();
    let policy = RenderingPolicy::plain();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
    let mut prompt = PromptUi::new(
        Locale::En,
        policy.stderr,
        Box::new(ScriptedKeys::choosing(0)),
        Box::new(RecordedScreen::new()),
    );
    let context = Context {
        location: &fixture.location,
        workspace_root: &fixture.workspace_root,
        clock: &clock,
        locale: Locale::En,
        can_prompt: false,
    };

    let code = super::exec(
        &Scope::Project(ProjectId::parse("example-org/example-repo")?),
        &context,
        &mut ui,
        &FakeSbx::listing(r#"{"sandboxes":[]}"#),
        &mut prompt,
    );

    assert_eq!(code, ExitCode::Failure);
    Ok(())
}
