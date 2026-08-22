use crate::cli::Interactivity;
use crate::commands::{Context, status::Scope};
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::project::ProjectId;
use crate::testing::global_status::FakeHost;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::Checked;
use crate::testing::project::Fixture;
use crate::testing::prompt::ScriptedPrompt;

use super::select_scope;

fn execute_prompt(
    fixture: &Fixture,
    host: &dyn crate::command::HostEnvironment,
    keys: ScriptedKeys,
    screen: &RecordedScreen,
) -> ExitCode {
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
            lang: Some(Locale::En),
            interactivity: Interactivity {
                stdin_is_tty: true,
                stderr_is_tty: true,
            },
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
