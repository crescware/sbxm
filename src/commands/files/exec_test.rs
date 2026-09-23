use std::fs;
use std::path::PathBuf;

use crate::commands::Context;
use crate::config::{self, ConfigState};
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;

use crate::testing::host::FakeSbx;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::Fixture;

use super::*;

/// 1回の実行で端末へ出たもの。
struct Ran {
    code: ExitCode,
    stdout: String,
    stderr: String,
    /// promptが描いた行。訊かなかった実行では空である。
    drawn: Vec<String>,
}

fn run(
    fixture: &Fixture,
    args: &Args,
    can_prompt: bool,
    keys: ScriptedKeys,
    host: &FakeSbx,
) -> Checked<Ran> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let screen = RecordedScreen::new();
    let policy = RenderingPolicy::plain();
    let code = {
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
            locale: Locale::En,
            can_prompt,
        };
        exec(args, &context, &mut ui, host, &mut prompt)
    };
    Ok(Ran {
        code,
        stdout: String::from_utf8(stdout).required_because("UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("UTF-8")?,
        drawn: screen.drawn(),
    })
}

/// home直下へ宣言候補のfileを置き、それを足す引数を返す。
fn adding(fixture: &Fixture, relative: &str) -> Checked<Args> {
    let file = fixture.dir.path().join(relative);
    fs::create_dir_all(file.parent().required()?).required()?;
    fs::write(&file, b"# notes\n").required()?;
    Ok(Args::Add {
        source: file,
        destination: None,
    })
}

fn nothing_running() -> FakeSbx {
    FakeSbx::listing(r#"{"sandboxes":[]}"#)
}

fn declared_count(fixture: &Fixture) -> Checked<usize> {
    match config::load(&fixture.location).required()? {
        ConfigState::Valid { config, .. } => Ok(config.files.len()),
        ConfigState::Missing => Ok(0),
    }
}

#[test]
fn listing_nothing_says_how_to_declare_a_file() -> Checked {
    let fixture = Fixture::new()?;
    let ran = run(
        &fixture,
        &Args::Ls,
        false,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;
    assert_eq!(ran.code, ExitCode::Success);
    assert!(
        ran.stdout.contains("No files are declared."),
        "{}",
        ran.stdout
    );
    assert!(
        ran.stdout.contains("sbxm files add <path>"),
        "{}",
        ran.stdout
    );
    Ok(())
}

#[test]
fn a_declaration_without_a_terminal_is_saved_and_the_way_to_place_it_is_shown() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    let host = nothing_running();

    let ran = run(
        &fixture,
        &adding(&fixture, ".claude/CLAUDE.md")?,
        false,
        ScriptedKeys::confirming(),
        &host,
    )?;

    assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
    assert!(ran.stdout.contains(".claude/CLAUDE.md"), "{}", ran.stdout);
    assert!(
        ran.stdout.contains("sbxm apply --files --all"),
        "{}",
        ran.stdout
    );
    assert!(ran.drawn.is_empty(), "nothing is asked without a terminal");
    assert!(
        host.calls().is_empty(),
        "no sandbox is touched: {:?}",
        host.calls()
    );
    assert_eq!(declared_count(&fixture)?, 1);

    // 足した宣言は一覧に並ぶ。
    let listed = run(
        &fixture,
        &Args::Ls,
        false,
        ScriptedKeys::confirming(),
        &host,
    )?;
    assert!(
        listed.stdout.contains(".claude/CLAUDE.md"),
        "{}",
        listed.stdout
    );
    Ok(())
}

#[test]
fn choosing_to_place_now_applies_the_declarations_to_every_project() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    let host = nothing_running();

    let ran = run(
        &fixture,
        &adding(&fixture, ".claude/CLAUDE.md")?,
        true,
        ScriptedKeys::confirming(),
        &host,
    )?;

    assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
    assert!(!ran.drawn.is_empty(), "the user is asked");
    // Sandboxの無い案件は、初回構築で宣言が置かれる。
    let row = ran
        .stdout
        .lines()
        .find(|line| line.contains("owner/repo"))
        .required_because("the project has a row")?;
    assert!(row.contains("not-created"), "{}", ran.stdout);
    assert!(host.ran("ls --json"));
    Ok(())
}

#[test]
fn choosing_later_or_leaving_the_question_keeps_the_declaration() -> Checked {
    for keys in [ScriptedKeys::choosing(1), ScriptedKeys::canceling()] {
        let fixture = Fixture::new()?;
        fixture.register("owner/repo")?;
        let host = nothing_running();

        let ran = run(
            &fixture,
            &adding(&fixture, ".claude/CLAUDE.md")?,
            true,
            keys,
            &host,
        )?;

        // 宣言は保存済みである。配置しなかったことは失敗ではない。
        assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
        assert!(
            ran.stdout.contains("sbxm apply --files --all"),
            "{}",
            ran.stdout
        );
        assert!(host.calls().is_empty(), "{:?}", host.calls());
        assert_eq!(declared_count(&fixture)?, 1);
    }
    Ok(())
}

#[test]
fn nothing_is_offered_when_no_project_is_registered_or_nothing_changed() -> Checked {
    let fixture = Fixture::new()?;
    let args = adding(&fixture, ".claude/CLAUDE.md")?;

    let ran = run(
        &fixture,
        &args,
        true,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;
    assert_eq!(ran.code, ExitCode::Success);
    assert!(ran.drawn.is_empty());
    assert!(!ran.stdout.contains("sbxm apply"), "{}", ran.stdout);

    fixture.register("owner/repo")?;
    let again = run(
        &fixture,
        &args,
        true,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;
    assert!(
        again.stdout.contains("already declared"),
        "{}",
        again.stdout
    );
    assert!(
        again.drawn.is_empty(),
        "an unchanged declaration is not placed again"
    );
    Ok(())
}

#[test]
fn a_name_that_often_holds_credentials_is_warned_about() -> Checked {
    let fixture = Fixture::new()?;
    let ran = run(
        &fixture,
        &adding(&fixture, ".config/tool/.env")?,
        false,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;
    assert_eq!(ran.code, ExitCode::Success);
    assert!(
        ran.stderr.contains("often holds credentials"),
        "{}",
        ran.stderr
    );
    Ok(())
}

#[test]
fn a_declaration_is_removed_and_a_refused_one_fails_the_run() -> Checked {
    let fixture = Fixture::new()?;
    run(
        &fixture,
        &adding(&fixture, ".claude/CLAUDE.md")?,
        false,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;

    let removing = Args::Rm {
        destination: ".claude/CLAUDE.md".to_string(),
    };
    let ran = run(
        &fixture,
        &removing,
        false,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;
    assert_eq!(ran.code, ExitCode::Success, "{}", ran.stderr);
    assert!(ran.stdout.contains("left as they are"), "{}", ran.stdout);
    assert_eq!(declared_count(&fixture)?, 0);

    let again = run(
        &fixture,
        &removing,
        false,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;
    assert_eq!(again.code, ExitCode::Failure);
    assert!(
        again.stderr.contains("file-not-declared"),
        "{}",
        again.stderr
    );

    let missing = Args::Add {
        source: PathBuf::from("/nonexistent-sbxm-directory/file.md"),
        destination: None,
    };
    let refused = run(
        &fixture,
        &missing,
        false,
        ScriptedKeys::confirming(),
        &nothing_running(),
    )?;
    assert_eq!(refused.code, ExitCode::Failure);
    Ok(())
}
