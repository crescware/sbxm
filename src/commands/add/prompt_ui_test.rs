use crate::design::PromptUi;
use crate::design::policy::StreamPolicy;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::i18n::Locale;

use crate::testing::outcome::{Checked, Required};

use super::IdentityPrompt;

fn prompt(keys: ScriptedKeys, screen: &RecordedScreen) -> PromptUi {
    PromptUi::new(
        Locale::En,
        StreamPolicy::plain(),
        Box::new(keys),
        Box::new(screen.clone()),
    )
}

#[test]
fn each_half_of_the_identity_is_asked_for_on_its_own_line() -> Checked {
    let screen = RecordedScreen::new();
    let name = prompt(ScriptedKeys::typing("Typed User"), &screen)
        .git_user_name("Host User")
        .required_because("the name is entered")?;

    assert_eq!(name, "Host UserTyped User", "the candidate is typed onto");
    assert_eq!(
        screen.lines().first().map(String::as_str),
        Some("Enter the name this project's commits are made under")
    );

    let screen = RecordedScreen::new();
    let email = prompt(ScriptedKeys::confirming(), &screen)
        .git_user_email("host@example.com")
        .required_because("the candidate is accepted")?;

    assert_eq!(email, "host@example.com");
    assert_eq!(
        screen.lines().first().map(String::as_str),
        Some("Enter the email address this project's commits are made under"),
        "the two halves do not share a heading"
    );
    Ok(())
}
