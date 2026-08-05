use crate::design::PromptUi;
use crate::design::policy::StreamPolicy;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::i18n::Locale;
use crate::msg;

use crate::testing::outcome::{Checked, Required};

use super::ProjectPrompt;

fn prompt(keys: ScriptedKeys) -> PromptUi {
    PromptUi::new(
        Locale::En,
        StreamPolicy::plain(),
        Box::new(keys),
        Box::new(RecordedScreen::new()),
    )
}

fn candidates() -> Vec<String> {
    ["owner/alpha", "owner/bravo", "owner/charlie"]
        .iter()
        .map(|label| (*label).to_string())
        .collect()
}

/// 案件の選択はtraitを通る。commandが見ているのはこちらであり、同名のinherent
/// methodではない。
#[test]
fn the_combined_open_selection_reaches_the_command_as_both_stopped_on_values() -> Checked {
    let chosen = ProjectPrompt::select_one(
        &mut prompt(ScriptedKeys::choosing(2)),
        &msg!("select-open-heading"),
        &candidates(),
    )
    .required_because("a project is chosen")?;
    assert_eq!(chosen, 2);

    let chosen = ProjectPrompt::select_many(
        &mut prompt(ScriptedKeys::checking(&[1, 2])),
        &msg!("select-stop-heading"),
        &candidates(),
    )
    .required_because("two projects are chosen")?;
    assert_eq!(chosen, vec![1, 2]);

    let chosen = ProjectPrompt::select_open(
        &mut prompt(ScriptedKeys::pressing(&[
            console::Key::ArrowDown,
            console::Key::ArrowRight,
            console::Key::Enter,
        ])),
        &msg!("select-open-heading"),
        &candidates(),
        31,
    )
    .required_because("a project and worktree index are chosen")?;
    assert_eq!(chosen, (1, 1));

    let mut maximums = |_project| Some(1);
    let chosen = ProjectPrompt::select_open_with_maximums(
        &mut prompt(ScriptedKeys::pressing(&[
            console::Key::ArrowRight,
            console::Key::ArrowRight,
            console::Key::Enter,
        ])),
        &msg!("select-open-heading"),
        &candidates(),
        31,
        &mut maximums,
    )
    .required_because("a calculated maximum is passed into the prompt")?;
    assert_eq!(chosen, (0, 1));
    Ok(())
}
