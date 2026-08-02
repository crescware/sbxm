use crate::design::PromptUi;
use crate::design::policy::StreamPolicy;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::i18n::Locale;

use crate::testing::outcome::{Checked, Required};

use super::ConfirmPrompt;

fn prompt(keys: ScriptedKeys) -> PromptUi {
    PromptUi::new(
        Locale::En,
        StreamPolicy::plain(),
        Box::new(keys),
        Box::new(RecordedScreen::new()),
    )
}

#[test]
fn only_the_whole_sandbox_name_confirms_the_deletion() -> Checked {
    let confirmed = prompt(ScriptedKeys::typing("owner-repo"))
        .confirm_sandbox_name("owner-repo")
        .required_because("the name was typed in full")?;
    assert!(confirmed);

    // 打ち間違いも、yesのつもりの入力も、確認にはならない。
    for typed in ["owner-rep", "y", "owner-repo-2", ""] {
        let confirmed = prompt(ScriptedKeys::typing(typed))
            .confirm_sandbox_name("owner-repo")
            .required_because("the prompt answered")?;
        assert!(!confirmed, "{typed:?} is not the sandbox name");
    }
    Ok(())
}

#[test]
fn the_spaces_around_a_pasted_name_are_not_a_mismatch() -> Checked {
    let confirmed = prompt(ScriptedKeys::typing("  owner-repo  "))
        .confirm_sandbox_name("owner-repo")
        .required_because("the prompt answered")?;
    assert!(confirmed);
    Ok(())
}
