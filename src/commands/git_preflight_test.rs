//! hostのgitを使う全commandが、案件選択や確認より先に古いgitを報告し、使わない
//! commandはgitのversionを読まない契約。
use std::cell::RefCell;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::{ExitCode, Result};
use crate::i18n::Locale;
use crate::support::git_cli::MINIMUM_HOST_GIT;
use crate::testing::global_status::FakeHost;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, https_repository, project_id};
use crate::testing::scripted_clock::ScriptedClock;

use super::{Command, Context, add, apply, destroy, files, guide, open, status};

/// 古いmacOS付属のgitが、versionを訊かれて返す出力。
const OLDER_GIT: &str = "git version 2.32.1 (Apple Git-133)\n";

struct RecordingHost {
    host: FakeHost,
    calls: RefCell<Vec<String>>,
}

impl RecordingHost {
    fn with_older_git() -> RecordingHost {
        RecordingHost {
            host: FakeHost::macos().responding("git --version", OLDER_GIT),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn git_calls(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("git "))
            .cloned()
            .collect()
    }
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

/// hostのgitを使いうるcommand。ローカル案件の保存や送り込み、登録、宣言fileの差分に使う。
fn git_commands(explicit: bool) -> Checked<Vec<Command>> {
    let project = explicit.then(|| project_id("owner/repo")).transpose()?;
    Ok(vec![
        Command::Add(add::Args {
            target: add::AddTarget::Clone(https_repository("owner/repo")?),
            worktrees: None,
            detach: None,
            git_identity: None,
        }),
        Command::Apply(apply::Args {
            project: project.clone(),
            all: false,
            files: false,
            force: false,
            worktrees: None,
        }),
        Command::Open(open::Args {
            project: project.clone(),
            index: None,
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
        Command::Files(files::Args::Pull {
            destination: ".claude/CLAUDE.md".to_string(),
            project: project.clone(),
        }),
        Command::Status(project.map_or(status::Scope::Prompt, status::Scope::Project)),
    ])
}

/// hostのgitを使わないcommand。
fn other_commands() -> Vec<Command> {
    vec![
        Command::Ls,
        Command::Guide(guide::Args {
            topic: None,
            project: None,
        }),
        Command::Files(files::Args::Ls),
        Command::Files(files::Args::Rm {
            destination: ".claude/CLAUDE.md".to_string(),
        }),
        Command::Status(status::Scope::Global),
    ]
}

fn execute(
    command: &Command,
    context: &Context,
    host: &dyn HostEnvironment,
    ui: &mut Ui,
    prompt: &mut PromptUi,
) -> ExitCode {
    match command {
        Command::Add(args) => add::exec(args, context, ui, host, prompt),
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
        Command::Help(_) | Command::Version(_) => {
            unreachable!("help and version run no host tool")
        }
    }
}

/// `command`を実行し、終了code、stdout、stderr、画面に描いたものを返す。
fn run(
    command: &Command,
    fixture: &Fixture,
    host: &RecordingHost,
    locale: Locale,
    explicit: bool,
) -> Checked<(ExitCode, Vec<u8>, String, RecordedScreen)> {
    let clock = ScriptedClock::default();
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
        execute(command, &context, host, &mut ui, &mut prompt)
    };
    Ok((code, stdout, String::from_utf8(stderr).required()?, screen))
}

#[test]
fn all_git_commands_report_an_older_git_before_any_prompt_or_mutation() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("owner/repo")?;
    let before = std::fs::read(project.paths.metadata_file()).required()?;
    let minimum = MINIMUM_HOST_GIT.to_string();
    for locale in [Locale::En, Locale::Ja] {
        for explicit in [false, true] {
            for command in git_commands(explicit)? {
                let host = RecordingHost::with_older_git();
                let (code, stdout, stderr, screen) =
                    run(&command, &fixture, &host, locale, explicit)?;
                assert_eq!(code, ExitCode::Failure, "{command:?}: {stderr}");
                assert!(stderr.contains("host-git-too-old"), "{command:?}: {stderr}");
                assert!(stderr.contains("2.32.1"), "{command:?}: {stderr}");
                assert!(stderr.contains(&minimum), "{command:?}: {stderr}");
                assert!(stdout.is_empty(), "{command:?}: no result or plan");
                assert!(
                    screen.drawn().is_empty(),
                    "{command:?}: no selection or confirmation"
                );
                // gitはversionだけを読み、ほかのgitは走らせない。
                assert_eq!(host.git_calls(), ["git --version"], "{command:?}");
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
fn commands_that_use_no_git_never_read_its_version() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    for command in other_commands() {
        let host = RecordingHost::with_older_git();
        let (_, _, stderr, _) = run(&command, &fixture, &host, Locale::En, true)?;
        assert!(host.git_calls().is_empty(), "{command:?}: {stderr}");
        assert!(
            !stderr.contains("host-git-too-old"),
            "{command:?}: {stderr}"
        );
    }
    Ok(())
}
