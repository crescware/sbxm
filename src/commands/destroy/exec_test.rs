use crate::boundary::host::HostEnvironment;
use crate::commands::Context;
use crate::design::prompt::{Key, RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;

use crate::testing::host::no_secrets;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, project_id};
use crate::testing::protection::{clean_host, commit_only_in_the_sandbox};

use super::super::Args;

/// `exec`が書いたstdoutとstderr、そして終了status。
struct Ran {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn run(fixture: &Fixture, host: &dyn HostEnvironment, keys: &[Key]) -> Checked<Ran> {
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
            force: false,
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
        host.ran("bundle create"),
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
    assert!(!host.ran("bundle create"), "{:?}", host.calls());
    assert!(ran.stdout.is_empty(), "{}", ran.stdout);
    Ok(())
}
