use console::Key;

use crate::design::policy::StreamPolicy;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::diagnostics::{ErrorId, ExitCode, Msg};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::testing::outcome::{Checked, Refused, Required};

use super::PromptUi;

fn labels() -> Vec<String> {
    ["owner/alpha", "owner/bravo", "owner/charlie"]
        .iter()
        .map(|label| (*label).to_string())
        .collect()
}

fn many_labels(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("owner/{index}")).collect()
}

fn prompt(keys: ScriptedKeys, screen: &RecordedScreen) -> PromptUi {
    PromptUi::new(
        Locale::En,
        StreamPolicy::plain(),
        Box::new(keys),
        Box::new(screen.clone()),
    )
}

fn heading() -> Msg {
    msg!("select-open-heading")
}

#[test]
fn a_project_and_worktree_index_are_confirmed_from_one_prompt() -> Checked {
    let screen = RecordedScreen::new();
    let keys = [Key::ArrowDown, Key::ArrowRight, Key::ArrowRight, Key::Enter];
    let chosen = prompt(ScriptedKeys::pressing(&keys), &screen)
        .select_open(&heading(), &labels(), 4)
        .required_because("the project and index are confirmed together")?;

    assert_eq!(chosen, (1, 2));
    assert_eq!(
        screen.lines(),
        vec!["✓ Selected owner/bravo, worktree index 2".to_string()]
    );
    Ok(())
}

#[test]
fn an_open_prompt_accepts_the_optimistic_index_bound() -> Checked {
    let screen = RecordedScreen::new();
    let keys = [Key::ArrowRight, Key::ArrowRight, Key::Enter];
    let chosen = prompt(ScriptedKeys::pressing(&keys), &screen)
        .select_open(&heading(), &labels(), 32)
        .required_because("the prompt is ready without metadata")?;

    assert_eq!(chosen, (0, 2));
    assert!(
        screen
            .drawn()
            .iter()
            .any(|line| line.contains("Worktree index: 2 (0-32)")),
        "the optimistic maximum is visible immediately: {:?}",
        screen.drawn()
    );
    Ok(())
}

#[test]
fn a_calculated_maximum_reduces_the_bound_while_the_prompt_is_open() -> Checked {
    let screen = RecordedScreen::new();
    let keys = [
        Key::ArrowRight,
        Key::ArrowRight,
        Key::ArrowRight,
        Key::Enter,
    ];
    let mut polls = 0;
    let mut maximums = |_project| {
        polls += 1;
        (polls >= 2).then_some(1)
    };
    let chosen = prompt(ScriptedKeys::pressing(&keys), &screen)
        .select_open_with_maximums(&heading(), &labels(), 31, &mut maximums)
        .required_because("the calculated maximum is applied before confirmation")?;

    assert_eq!(chosen, (0, 1));
    assert!(
        screen
            .drawn()
            .iter()
            .any(|line| line.contains("Worktree index: 1 (0-1)")),
        "the calculated maximum is rendered: {:?}",
        screen.drawn()
    );
    Ok(())
}

#[test]
fn only_the_confirmed_value_is_left_where_the_list_was() -> Checked {
    let screen = RecordedScreen::new();
    let chosen = prompt(ScriptedKeys::choosing(1), &screen)
        .select_one(&heading(), &labels())
        .required_because("the second candidate is confirmed")?;

    assert_eq!(chosen, 1);
    assert_eq!(
        screen.lines(),
        vec!["\u{2713} Selected owner/bravo".to_string()],
        "the list is taken back down and the answer stays"
    );
    assert!(screen.cursor_is_visible(), "the cursor is handed back");
    Ok(())
}

#[test]
fn every_checked_row_is_confirmed_together() -> Checked {
    let screen = RecordedScreen::new();
    let chosen = prompt(ScriptedKeys::checking(&[0, 2]), &screen)
        .select_many(&heading(), &labels())
        .required_because("two candidates are confirmed")?;

    assert_eq!(chosen, vec![0, 2]);
    assert_eq!(
        screen.lines(),
        vec!["\u{2713} Selected owner/alpha, owner/charlie".to_string()],
        "the answer names every chosen candidate"
    );
    Ok(())
}

#[test]
fn a_cancelled_selection_leaves_nothing_behind() -> Checked {
    let screen = RecordedScreen::new();
    let error = prompt(ScriptedKeys::canceling(), &screen)
        .select_one(&heading(), &labels())
        .refused_because("Esc changes nothing")?;

    assert_eq!(error.exit_code(), ExitCode::Canceled);
    assert!(screen.lines().is_empty(), "{:?}", screen.lines());
    assert!(screen.cursor_is_visible());
    Ok(())
}

#[test]
fn an_empty_candidate_list_is_unresolved_rather_than_an_empty_prompt() -> Checked {
    let screen = RecordedScreen::new();
    let error = prompt(ScriptedKeys::confirming(), &screen)
        .select_one(&heading(), &[])
        .refused_because("there is nothing to choose")?;

    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    assert_ne!(error.exit_code(), ExitCode::Canceled);
    assert!(screen.drawn().is_empty(), "nothing is drawn");

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal is reported")?;
    let described = Catalog::new(Locale::En)
        .format(&diagnostic.description)
        .required_because("the report is readable")?;
    assert!(described.contains(" 0 candidates"), "{described}");
    Ok(())
}

#[test]
fn a_screen_whose_height_is_unknown_shows_every_candidate() -> Checked {
    let screen = RecordedScreen::new();
    prompt(ScriptedKeys::confirming(), &screen)
        .select_one(&heading(), &many_labels(10))
        .required_because("the first candidate is confirmed")?;

    assert_eq!(candidates_drawn(&screen), 10);
    Ok(())
}

#[test]
fn a_short_screen_shows_a_window_of_the_candidates() -> Checked {
    // heading、操作説明、空行、結果の一行ぶんを残した4行だけが一覧に使える。
    let screen = RecordedScreen::with_rows(10);
    prompt(ScriptedKeys::confirming(), &screen)
        .select_one(&heading(), &many_labels(10))
        .required_because("the first candidate is confirmed")?;

    assert_eq!(candidates_drawn(&screen), 4);
    Ok(())
}

/// 1画面ぶんに描かれた候補の数。確定の一行は候補ではない。
fn candidates_drawn(screen: &RecordedScreen) -> usize {
    screen
        .drawn()
        .iter()
        .filter(|line| line.contains("owner/") && !line.contains("Selected"))
        .count()
}

#[test]
fn a_screen_that_cannot_be_written_stops_the_prompt() -> Checked {
    let screen = RecordedScreen::failing(std::io::ErrorKind::BrokenPipe);
    let error = prompt(ScriptedKeys::confirming(), &screen)
        .select_one(&heading(), &labels())
        .refused_because("a prompt that cannot be drawn cannot be answered")?;

    assert_eq!(error.first_id(), Some(ErrorId::PromptUnreadable));
    Ok(())
}

#[test]
fn keys_that_cannot_be_read_stop_the_prompt() -> Checked {
    let screen = RecordedScreen::new();
    let error = prompt(
        ScriptedKeys::failing(std::io::ErrorKind::BrokenPipe),
        &screen,
    )
    .select_one(&heading(), &labels())
    .refused_because("a prompt that cannot be read cannot be answered")?;

    assert_eq!(error.first_id(), Some(ErrorId::PromptUnreadable));
    Ok(())
}

#[test]
fn a_candidate_is_placed_in_the_field_and_can_be_confirmed_as_it_is() -> Checked {
    let screen = RecordedScreen::new();
    let typed = prompt(ScriptedKeys::confirming(), &screen)
        .input(&msg!("prompt-git-user-name"), "Host User")
        .required_because("Enter alone confirms the candidate")?;

    assert_eq!(typed, "Host User");
    assert_eq!(
        screen.lines(),
        vec![
            "Enter the name this project's commits are made under".to_string(),
            "Host User".to_string(),
        ],
        "the candidate is shown as the line being edited"
    );
    Ok(())
}

#[test]
fn the_candidate_can_be_typed_over() -> Checked {
    let screen = RecordedScreen::new();
    let mut keys = vec![Key::Backspace; "Host User".chars().count()];
    keys.extend("Typed".chars().map(Key::Char));
    keys.push(Key::Enter);

    let typed = prompt(ScriptedKeys::pressing(&keys), &screen)
        .input(&msg!("prompt-git-user-name"), "Host User")
        .required_because("the candidate is not a decided value")?;

    assert_eq!(typed, "Typed");
    assert_eq!(
        screen.lines().last().map(String::as_str),
        Some("Typed"),
        "what was rubbed out is gone from the line as well"
    );
    Ok(())
}

#[test]
fn backspace_clears_one_wide_character_from_the_screen() -> Checked {
    let screen = RecordedScreen::new();
    let keys = [Key::Char('a'), Key::Char('界'), Key::Backspace, Key::Enter];
    let typed = prompt(ScriptedKeys::pressing(&keys), &screen)
        .exact(&msg!("destroy-confirm-prompt"))
        .required_because("the wide character is removed")?;

    assert_eq!(typed, "a");
    assert_eq!(
        screen.lines().last().map(String::as_str),
        Some("a"),
        "display width is not confused with the number of Rust chars"
    );
    Ok(())
}

#[test]
fn an_exact_answer_starts_from_an_empty_field() -> Checked {
    let screen = RecordedScreen::new();
    let typed = prompt(ScriptedKeys::typing("owner-repo"), &screen)
        .exact(&msg!("destroy-confirm-prompt"))
        .required_because("the name is typed in full")?;

    assert_eq!(typed, "owner-repo");
    assert_eq!(
        screen.lines(),
        vec![
            "Type the sandbox name to confirm the deletion".to_string(),
            "owner-repo".to_string(),
        ]
    );
    Ok(())
}

#[test]
fn an_input_takes_only_the_keys_it_needs() -> Checked {
    let screen = RecordedScreen::new();
    // 行編集は提供しない。受け付けない打鍵は入力にも画面にも残らない。
    let keys = [
        Key::Tab,
        Key::ArrowLeft,
        Key::Char('a'),
        Key::Home,
        Key::Enter,
    ];
    let typed = prompt(ScriptedKeys::pressing(&keys), &screen)
        .exact(&msg!("destroy-confirm-prompt"))
        .required_because("the answer is confirmed")?;

    assert_eq!(typed, "a");
    Ok(())
}

#[test]
fn a_cancelled_input_returns_nothing_that_could_be_taken_as_an_answer() -> Checked {
    for key in [Key::Escape, Key::CtrlC] {
        let screen = RecordedScreen::new();
        let error = prompt(ScriptedKeys::pressing(std::slice::from_ref(&key)), &screen)
            .exact(&msg!("destroy-confirm-prompt"))
            .refused_because("Esc and Ctrl-C change nothing")?;

        assert_eq!(error.exit_code(), ExitCode::Canceled, "{key:?}");
    }
    Ok(())
}

#[test]
fn the_language_the_prompt_asks_in_follows_the_one_that_was_settled_on() -> Checked {
    let screen = RecordedScreen::new();
    let mut prompt = prompt(ScriptedKeys::confirming(), &screen);
    prompt.set_locale(Locale::Ja);
    prompt
        .exact(&msg!("destroy-confirm-prompt"))
        .required_because("the answer is confirmed")?;

    assert_eq!(
        screen.lines().first().map(String::as_str),
        Some("削除を確認するため、Sandbox名を入力してください")
    );
    Ok(())
}
