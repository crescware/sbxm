//! 認証が必要な全commandが、案件選択や確認より先に未loginを報告する契約。
use std::cell::RefCell;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::{ExitCode, Result};
use crate::i18n::Locale;
use crate::testing::global_status::FakeHost;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, project_id};

use super::{Command, Context, apply, destroy, guide, open, status};

struct RecordingHost {
    host: FakeHost,
    calls: RefCell<Vec<String>>,
}

impl HostEnvironment for RecordingHost {
    fn command_exists(&self, program: &str) -> bool {
        self.host.command_exists(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls
            .borrow_mut()
            .push(format!("{} {}", spec.program, spec.args.join(" ")));
        self.host.run(spec)
    }
}

fn commands(explicit: bool) -> Checked<Vec<Command>> {
    let project = explicit.then(|| project_id("owner/repo")).transpose()?;
    Ok(vec![
        Command::Open(open::Args {
            project: project.clone(),
            index: None,
        }),
        Command::Open(open::Args {
            project: project.clone(),
            index: Some(0),
        }),
        Command::Apply(apply::Args {
            project: project.clone(),
            files: true,
            worktrees: None,
        }),
        Command::Guide(guide::Args {
            topic: explicit.then_some(guide::Topic::CredentialRotation),
            project: project.clone(),
        }),
        Command::Repair(project.clone()),
        Command::Rebuild(project.clone()),
        Command::Stop(project.clone().into_iter().collect()),
        Command::Destroy(destroy::Args {
            project: project.clone(),
            force: false,
        }),
        Command::Destroy(destroy::Args {
            project: project.clone(),
            force: true,
        }),
        Command::Ls,
        Command::Status(project.map_or(status::Scope::Prompt, status::Scope::Project)),
    ])
}

fn execute(
    command: &Command,
    context: &Context,
    host: &dyn HostEnvironment,
    ui: &mut Ui,
    prompt: &mut PromptUi,
) -> ExitCode {
    match command {
        Command::Open(args) => open::exec(args, context, ui, host, prompt),
        Command::Apply(args) => apply::exec(args, context, ui, host, prompt),
        Command::Guide(args) => guide::exec(args, context, ui, host, prompt),
        Command::Repair(project) => {
            super::repair::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Rebuild(project) => {
            super::rebuild::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Stop(projects) => super::stop::exec(projects, context, ui, host, prompt),
        Command::Destroy(args) => destroy::exec(args, context, ui, host, prompt),
        Command::Ls => super::ls::exec(context, ui, host),
        Command::Status(scope) => status::exec(scope, context, ui, host, prompt),
        _ => unreachable!("this test covers commands that require Docker authentication"),
    }
}

#[test]
fn all_sandbox_commands_report_missing_login_before_any_prompt_or_mutation() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("owner/repo")?;
    let before = std::fs::read(project.paths.metadata_file()).required()?;
    for locale in [Locale::En, Locale::Ja] {
        for explicit in [false, true] {
            for command in commands(explicit)? {
                let host = RecordingHost {
                    host: FakeHost::macos().failing(
                        "sbx ls --json",
                        "ERROR: list sandboxes: list local runtimes: list runtimes: request failed: 401 Unauthorized: user is not authenticated to Docker: secret not found\nno valid user session found, please sign in to Docker to proceed\n\nSign in with: sbx login\n",
                        1,
                    ),
                    calls: RefCell::new(Vec::new()),
                };
                let screen = RecordedScreen::new();
                let policy = RenderingPolicy::plain();
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                let code = {
                    let mut ui = Ui::capture(locale, policy, &mut stdout, &mut stderr);
                    let mut prompt = PromptUi::new(
                        locale,
                        policy.stderr,
                        Box::new(ScriptedKeys::canceling()),
                        Box::new(screen.clone()),
                    );
                    let context = Context {
                        location: &fixture.location,
                        workspace_root: &fixture.workspace_root,
                        locale,
                        can_prompt: !explicit,
                    };
                    execute(&command, &context, &host, &mut ui, &mut prompt)
                };
                let stderr = String::from_utf8(stderr).required()?;
                assert_eq!(code, ExitCode::Failure, "{command:?}: {stderr}");
                assert!(
                    stderr.contains("sbx-login-missing"),
                    "{command:?}: {stderr}"
                );
                assert!(stderr.contains("sbx login"), "{command:?}: {stderr}");
                assert!(
                    !stderr.contains("external-command-failed"),
                    "{command:?}: {stderr}"
                );
                assert!(stdout.is_empty(), "{command:?}: no result or plan");
                assert!(
                    screen.drawn().is_empty(),
                    "{command:?}: no selection or confirmation"
                );
                assert_eq!(*host.calls.borrow(), ["sbx ls --json"], "{command:?}");
                assert_eq!(
                    std::fs::read(project.paths.metadata_file()).required()?,
                    before
                );
            }
        }
    }
    Ok(())
}

#[test]
fn authenticated_commands_reach_selection_and_can_be_canceled() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    for command in commands(false)? {
        if matches!(command, Command::Ls) {
            continue;
        }
        let host = RecordingHost {
            host: FakeHost::macos(),
            calls: RefCell::new(Vec::new()),
        };
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
                locale: Locale::En,
                can_prompt: true,
            };
            execute(&command, &context, &host, &mut ui, &mut prompt)
        };
        assert_eq!(
            code,
            ExitCode::Canceled,
            "{command:?}: {:?}",
            String::from_utf8_lossy(&stderr)
        );
        assert!(!screen.drawn().is_empty(), "{command:?}");
        assert_eq!(*host.calls.borrow(), ["sbx ls --json"], "{command:?}");
    }
    Ok(())
}
