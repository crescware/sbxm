use crate::design::PromptUi;
use crate::design::policy::StreamPolicy;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::i18n::Locale;
use crate::msg;

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
fn the_typed_line_is_returned_as_it_was_read() -> Checked {
    let typed = prompt(ScriptedKeys::typing("owner-repo"))
        .read_sandbox_name(&msg!("destroy-confirm-prompt"))
        .required_because("the prompt answered")?;
    assert_eq!(typed, "owner-repo");
    Ok(())
}

#[test]
fn surrounding_whitespace_from_a_paste_is_not_part_of_the_answer() -> Checked {
    let typed = prompt(ScriptedKeys::typing("  owner-repo  "))
        .read_sandbox_name(&msg!("destroy-confirm-prompt"))
        .required_because("the prompt answered")?;
    assert_eq!(typed, "owner-repo");
    Ok(())
}
