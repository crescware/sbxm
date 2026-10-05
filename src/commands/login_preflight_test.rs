//! 認証が必要な全commandが、案件選択や確認より先に未loginを報告する契約。
use std::cell::RefCell;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::{ExitCode, Result};
use crate::i18n::Locale;
use crate::testing::global_status::FakeHost;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, project_id};
use crate::testing::scripted_clock::ScriptedClock;

use super::{Command, Context, apply, destroy, files, guide, open, status};

/// `files pull`が取り出す、宣言済みの配置先。
const DECLARED: &str = ".claude/CLAUDE.md";

struct RecordingHost {
    host: FakeHost,
    calls: RefCell<Vec<String>>,
}

impl AnsweredHost for RecordingHost {
    fn has_command(&self, program: &str) -> bool {
        self.host.command_exists(program)
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
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
            all: false,
            files: true,
            force: false,
            worktrees: None,
        }),
        // 全案件への配置も、案件を1件も読む前に認証を確かめる。
        Command::Apply(apply::Args {
            project: None,
            all: true,
            files: true,
            force: false,
            worktrees: None,
        }),
        Command::Guide(guide::Args {
            topic: explicit.then_some(guide::Topic::CredentialRotation),
            project: project.clone(),
        }),
        Command::Repair(project.clone()),
        Command::Sync(project.clone()),
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
        // Sandboxから宣言fileを取り出す実行も、宣言を読む前に認証を確かめる。
        Command::Files(files::Args::Pull {
            destination: DECLARED.to_string(),
            project: project.clone(),
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
        Command::Sync(project) => super::sync::exec(project.as_ref(), context, ui, host, prompt),
        Command::Guide(args) => guide::exec(args, context, ui, host, prompt),
        Command::Repair(project) => {
            super::repair::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Rebuild(project) => {
            super::rebuild::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Stop(projects) => super::stop::exec(projects, context, ui, host, prompt),
        Command::Destroy(args) => destroy::exec(args, context, ui, host, prompt),
        Command::Files(args) => files::exec(args, context, ui, host, prompt),
        Command::Ls => super::ls::exec(context, ui, host),
        Command::Status(scope) => status::exec(scope, context, ui, host, prompt),
        _ => unreachable!("this test covers commands that require Docker authentication"),
    }
}

#[test]
fn all_sandbox_commands_report_missing_login_before_any_prompt_or_mutation() -> Checked {
    let clock = ScriptedClock::default();
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
                        clock: &clock,
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
fn all_sandbox_commands_flush_the_login_phase_before_waiting_in_both_languages() -> Checked {
    use std::cell::Cell;
    use std::io::BufWriter;
    use std::rc::Rc;

    use crate::testing::command::ScriptedWriter;
    use crate::testing::host::ChangingBefore;

    let clock = ScriptedClock::default();
    let fixture = Fixture::new()?;
    for (locale, expected) in [
        (Locale::En, "Checking Docker Sandboxes sign-in."),
        (Locale::Ja, "Docker Sandboxesへのloginを確認します。"),
    ] {
        for command in commands(false)? {
            let stdout = ScriptedWriter::accepting_all();
            let stderr = ScriptedWriter::accepting_all();
            let visible_stdout = stdout.written();
            let visible_stderr = stderr.written();
            let observed = Rc::new(Cell::new(false));
            let observed_by_host = observed.clone();
            let host = ChangingBefore::new(
                FakeHost::macos().failing(
                    "sbx ls --json",
                    "401 Unauthorized: user is not authenticated to Docker",
                    1,
                ),
                "ls --json",
                move || {
                    let progress = String::from_utf8_lossy(&visible_stderr.borrow()).into_owned();
                    assert!(progress.contains(expected), "{progress}");
                    assert!(visible_stdout.borrow().is_empty(), "no result or plan yet");
                    observed_by_host.set(true);
                },
            );
            let policy = RenderingPolicy::plain();
            let screen = RecordedScreen::new();
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
                Box::new(screen.clone()),
            );
            let context = Context {
                location: &fixture.location,
                workspace_root: &fixture.workspace_root,
                clock: &clock,
                locale,
                can_prompt: true,
            };
            assert_eq!(
                execute(&command, &context, &host, &mut ui, &mut prompt),
                ExitCode::Failure,
                "{command:?}"
            );
            assert!(observed.get(), "{command:?}: the preflight was not reached");
            assert!(screen.drawn().is_empty(), "{command:?}: no prompt yet");
        }
    }
    Ok(())
}

#[test]
fn authenticated_commands_reach_selection_and_can_be_canceled() -> Checked {
    let clock = ScriptedClock::default();
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    // `sync`は`--local`の案件だけを並べる。選ぶ案件が無ければ、選ばせる前に断る。
    fixture.register_local("/srv/code/app/.git", "app")?;
    // `files pull`は宣言したfileだけを取り出す。宣言が無ければ、選ばせる前に断る。
    let source = fixture.dir.path().join(DECLARED);
    std::fs::create_dir_all(source.parent().required()?).required()?;
    std::fs::write(&source, b"# notes\n").required()?;
    files::add(
        &fixture.location,
        &crate::config::GlobalConfig::default(),
        &files::absolute_source(&source).required()?,
        None,
    )
    .required_because("declare the file to pull")?;
    for command in commands(false)? {
        // 全案件が対象の実行には、選ぶ案件が無い。
        if matches!(
            command,
            Command::Ls | Command::Apply(apply::Args { all: true, .. })
        ) {
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
                clock: &clock,
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
