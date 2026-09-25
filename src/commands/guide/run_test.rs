use crate::commands::guide::{Args, Topic};
use crate::diagnostics::ErrorId;
use crate::i18n::Locale;
use crate::testing::host::{FakeSbx, custom_secret_listing};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::{Fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;

fn explicit(project: &str) -> Checked<Args> {
    Ok(Args {
        topic: Some(Topic::CredentialRotation),
        project: Some(project_id(project)?),
    })
}

#[test]
fn explicit_topic_and_project_start_without_a_prompt() -> Checked {
    let fixture = Fixture::new()?;
    let registered = fixture.register("owner/repo")?;
    let sandbox = registered.metadata.sandbox_name();
    let listing = custom_secret_listing(sandbox.as_str(), "sbx-cs-existing");
    let host = FakeSbx::listing("").answering("secret ls", 0, &listing);
    let mut prompt = ScriptedPrompt::choosing(0);

    let output = super::run::run(
        &explicit("owner/repo")?,
        &fixture.location,
        Locale::En,
        &host,
        &mut prompt,
    )?;

    assert!(prompt.asked.borrow().is_empty());
    assert_eq!(output.project, "owner/repo");
    assert_eq!(output.sandbox, sandbox.as_str());
    assert!(
        output
            .register_command
            .contains("--placeholder sbx-cs-existing")
    );
    assert!(output.register_command.ends_with("--value <token>"));
    assert!(
        !output.register_command.contains("ghp_example"),
        "the SECRET column is never copied into guidance"
    );
    Ok(())
}

#[test]
fn omitted_values_prompt_for_the_topic_before_the_project() -> Checked {
    let fixture = Fixture::new()?;
    let registered = fixture.register("owner/repo")?;
    let sandbox = registered.metadata.sandbox_name();
    let listing = custom_secret_listing(sandbox.as_str(), "sbx-cs-existing");
    let host = FakeSbx::listing("").answering("secret ls", 0, &listing);
    let mut prompt = ScriptedPrompt::choosing(0);

    super::run::run(
        &Args {
            topic: None,
            project: None,
        },
        &fixture.location,
        Locale::En,
        &host,
        &mut prompt,
    )?;

    assert_eq!(
        *prompt.headings.borrow(),
        ["select-guide-topic-heading", "select-guide-project-heading"]
    );
    assert_eq!(
        prompt.asked.borrow()[0],
        ["Rotate a GitHub credential".to_string()]
    );
    assert_eq!(prompt.asked.borrow()[1], ["owner/repo".to_string()]);
    Ok(())
}

#[test]
fn an_explicit_topic_prompts_only_for_the_project() -> Checked {
    let fixture = Fixture::new()?;
    let registered = fixture.register("owner/repo")?;
    let sandbox = registered.metadata.sandbox_name();
    let listing = custom_secret_listing(sandbox.as_str(), "sbx-cs-existing");
    let host = FakeSbx::listing("").answering("secret ls", 0, &listing);
    let mut prompt = ScriptedPrompt::choosing(0);

    super::run::run(
        &Args {
            topic: Some(Topic::CredentialRotation),
            project: None,
        },
        &fixture.location,
        Locale::En,
        &host,
        &mut prompt,
    )?;

    assert_eq!(*prompt.headings.borrow(), ["select-guide-project-heading"]);
    Ok(())
}

#[test]
fn a_global_registration_is_replaced_in_its_existing_scope() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    let listing = custom_secret_listing("(global)", "sbx-cs-global");
    let host = FakeSbx::listing("").answering("secret ls", 0, &listing);

    let output = super::run::run(
        &explicit("owner/repo")?,
        &fixture.location,
        Locale::En,
        &host,
        &mut ScriptedPrompt::choosing(0),
    )?;

    assert!(
        output
            .register_command
            .starts_with("sbx secret set-custom --host 'github.com'"),
        "global scope is the set-custom default: {}",
        output.register_command
    );
    assert!(!output.register_command.contains("(global)"));
    Ok(())
}

#[test]
fn a_missing_registration_uses_the_existing_secret_diagnostic() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    let host = FakeSbx::listing("").answering("secret ls", 0, "No secrets found\n");

    let error = super::run::run(
        &explicit("owner/repo")?,
        &fixture.location,
        Locale::En,
        &host,
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("there is no existing credential to rotate")?;

    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    let command = error.diagnostics()[0]
        .remediation
        .as_ref()
        .and_then(|remediation| remediation.commands.first())
        .map(crate::design::text::CommandLine::as_str)
        .required_because("the missing registration can be created")?;
    assert!(!command.contains("--placeholder"));
    assert!(command.ends_with("--value <token>"));
    Ok(())
}

#[test]
fn an_unresolved_topic_selection_stops_before_selecting_a_project() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    let host = FakeSbx::listing("");
    let mut prompt = ScriptedPrompt::choosing(Topic::ALL.len());

    let error = super::run::run(
        &Args {
            topic: None,
            project: None,
        },
        &fixture.location,
        Locale::En,
        &host,
        &mut prompt,
    )
    .refused_because("the prompt returned no declared topic")?;

    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    assert_eq!(*prompt.headings.borrow(), ["select-guide-topic-heading"]);
    Ok(())
}

#[test]
fn canceling_the_topic_selection_cancels_the_guide() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("owner/repo")?;
    let result = super::run::run(
        &Args {
            topic: None,
            project: None,
        },
        &fixture.location,
        Locale::En,
        &FakeSbx::listing(""),
        &mut ScriptedPrompt::canceling(),
    );

    assert!(matches!(result, Err(crate::diagnostics::Error::Canceled)));
    Ok(())
}

#[test]
fn a_local_project_has_no_token_to_rotate() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register_local("/srv/code/app/.git", "app")?;
    let host = FakeSbx::listing("");

    let error = super::run::run(
        &explicit("local/app")?,
        &fixture.location,
        Locale::En,
        &host,
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("a host repository uses no GitHub token")?;

    assert_eq!(error.first_id(), Some(ErrorId::NoGithubToken));
    assert!(host.calls().is_empty(), "{:?}", host.calls());
    Ok(())
}
