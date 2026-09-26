use crate::boundary::host::HostEnvironment;
use crate::commands::Context;
use crate::design::prompt::{Key, RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;

use crate::testing::host::{FakeSbx, no_secrets};
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::protection::{clean_host, commit_only_in_the_sandbox};

use super::super::Args;

/// `exec`が書いたstdoutとstderr、そして終了status。
struct Ran {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn run(fixture: &Fixture, host: &dyn HostEnvironment, keys: &[Key]) -> Checked<Ran> {
    run_with(fixture, host, keys, false)
}

fn run_with(
    fixture: &Fixture,
    host: &dyn HostEnvironment,
    keys: &[Key],
    force: bool,
) -> Checked<Ran> {
    let policy = RenderingPolicy::plain();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(ScriptedKeys::pressing(keys)),
            Box::new(RecordedScreen::new()),
        );
        let context = Context {
            location: &fixture.location,
            workspace_root: &fixture.workspace_root,
            locale: Locale::En,
            can_prompt: true,
        };
        let args = Args {
            project: Some(project_id("example-org/example-repo")?),
            force,
        };
        super::exec(&args, &context, &mut ui, host, &mut prompt)
    };
    Ok(Ran {
        code,
        stdout: String::from_utf8(stdout).required_because("destroy stdout is UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("destroy stderr is UTF-8")?,
    })
}

#[test]
fn a_commit_saved_to_the_host_on_request_lets_the_plan_be_drawn() -> Checked {
    // 保護の検査が止めたあと、hostへ保存する選択をすれば、同じ案件をもう一度準備して
    // 計画と確認へ進む。確認はcancelし、削除には進ませない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(
        commit_only_in_the_sandbox(clean_host(&fixture, &project)?, &project),
        project.sandbox.as_str(),
    );

    let ran = run(&fixture, &host, &[Key::Enter, Key::Escape])?;

    assert_eq!(ran.code, ExitCode::Canceled, "{}{}", ran.stdout, ran.stderr);
    assert!(
        ran.stderr.contains("origin-commit-unreachable"),
        "the refusal is shown before the offer: {}",
        ran.stderr
    );
    assert!(
        host.ran(crate::support::bundle::PLACE_SAVE_REFS),
        "the commits are saved to the host: {:?}",
        host.calls()
    );
    assert!(
        ran.stdout.contains("These are deleted:"),
        "the plan is drawn once the commits are saved: {}",
        ran.stdout
    );
    assert!(!host.ran("rm "), "{:?}", host.calls());
    Ok(())
}

#[test]
fn choosing_not_to_save_stops_before_the_plan() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(
        commit_only_in_the_sandbox(clean_host(&fixture, &project)?, &project),
        project.sandbox.as_str(),
    );

    let ran = run(&fixture, &host, &[Key::ArrowDown, Key::Enter])?;

    assert_eq!(ran.code, ExitCode::Failure, "{}{}", ran.stdout, ran.stderr);
    assert!(
        !host.ran(crate::support::bundle::PLACE_SAVE_REFS),
        "{:?}",
        host.calls()
    );
    assert!(ran.stdout.is_empty(), "{}", ran.stdout);
    Ok(())
}

/// 削除が最後まで進む一覧。login確認、準備、削除直前の観測では対象があり、削除後には
/// 消えている。
fn removable(fixture: &Fixture, project: &Registered, host: FakeSbx) -> Checked<FakeSbx> {
    let present = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(project, "running")?
    );
    *host.listing.borrow_mut() = vec![
        r#"{"sandboxes":[]}"#.to_string(),
        present.clone(),
        present.clone(),
        present,
    ];
    Ok(host)
}

/// 打った文字列をEnterで確定する打鍵。
fn typed(text: &str) -> Vec<Key> {
    let mut keys: Vec<Key> = text.chars().map(Key::Char).collect();
    keys.push(Key::Enter);
    keys
}

#[test]
fn a_confirmed_destroy_removes_the_sandbox_and_reports_the_project_as_unmanaged() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = removable(
        &fixture,
        &project,
        no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str()),
    )?;

    let ran = run(&fixture, &host, &typed("example-org/example-repo"))?;

    assert_eq!(ran.code, ExitCode::Success, "{}{}", ran.stdout, ran.stderr);
    assert!(
        host.ran(&format!("rm {}", project.sandbox)),
        "{:?}",
        host.calls()
    );
    assert!(
        ran.stdout.contains("is no longer managed"),
        "{}",
        ran.stdout
    );
    assert!(!project.paths.metadata_file().exists());
    Ok(())
}

#[test]
fn a_forced_destroy_warns_and_removes_without_asking() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = removable(
        &fixture,
        &project,
        no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str()),
    )?;

    // 強制削除は保護の検査を行わないため、削除直前の観測が1回少ない。
    host.listing.borrow_mut().pop();
    let ran = run_with(&fixture, &host, &[], true)?;

    assert_eq!(ran.code, ExitCode::Success, "{}{}", ran.stdout, ran.stderr);
    assert!(
        ran.stderr.contains("Force mode skips"),
        "the bypass is announced: {}",
        ran.stderr
    );
    assert!(
        host.ran(&format!("rm --force {}", project.sandbox)),
        "{:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_project_that_is_not_managed_is_reported_before_anything_is_inspected() -> Checked {
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let ran = run(&fixture, &host, &[])?;

    assert_eq!(ran.code, ExitCode::Failure, "{}{}", ran.stdout, ran.stderr);
    assert!(ran.stdout.is_empty(), "{}", ran.stdout);
    assert!(!host.ran("exec "), "{:?}", host.calls());
    Ok(())
}

#[test]
fn a_refusal_that_remains_after_saving_is_reported_without_asking_again() -> Checked {
    // 打鍵は1問分しか用意しない。2度目を訊けば、打鍵が尽きた失敗として現れる。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(
        commit_only_in_the_sandbox(clean_host(&fixture, &project)?, &project),
        project.sandbox.as_str(),
    )
    .answering_in_turn(
        &format!(
            "for-each-ref --format=%(refname) %(objectname) refs/sbx/{}/",
            project.sandbox.as_str()
        ),
        &[(0, "")],
    );

    let ran = run(&fixture, &host, &[Key::Enter])?;

    assert_eq!(ran.code, ExitCode::Failure, "{}{}", ran.stdout, ran.stderr);
    assert_eq!(
        ran.stderr.matches("origin-commit-unreachable").count(),
        2,
        "{}",
        ran.stderr
    );
    assert!(!ran.stderr.contains("prompt-unreadable"), "{}", ran.stderr);
    assert!(!host.ran("rm "), "{:?}", host.calls());
    Ok(())
}
