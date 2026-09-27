use crate::design::policy::StreamPolicy;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::diagnostics::{ErrorId, ExitCode, Msg};
use crate::i18n::{Catalog, Locale};
use crate::msg;
use crate::testing::outcome::{Checked, Refused, Required};

use super::{Key, PromptUi};

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
        .select_open(&heading(), &labels(), &[Some(4); 3])
        .required_because("the project and index are confirmed together")?;

    assert_eq!(chosen, (1, 2));
    assert_eq!(
        screen.lines(),
        vec!["✓ Selected owner/bravo, worktree index 2".to_string()]
    );
    Ok(())
}

#[test]
fn the_range_follows_the_project_under_the_cursor() -> Checked {
    let screen = RecordedScreen::new();
    let keys = [
        Key::ArrowRight,
        Key::ArrowRight,
        Key::ArrowRight,
        Key::ArrowDown,
        Key::Enter,
    ];
    let chosen = prompt(ScriptedKeys::pressing(&keys), &screen)
        .select_open(&heading(), &labels(), &[Some(3), Some(1), Some(0)])
        .required_because("each project brings its own range")?;

    assert_eq!(
        chosen,
        (1, 1),
        "the index is held within the project it lands on"
    );
    let drawn = screen.drawn();
    assert!(
        drawn
            .iter()
            .any(|line| line.contains("Worktree index: 3 (0-3)")),
        "the first project's range is shown from the start: {drawn:?}"
    );
    assert!(
        drawn
            .iter()
            .any(|line| line.contains("Worktree index: 1 (0-1)")),
        "the range changes with the project: {drawn:?}"
    );
    assert!(
        !drawn
            .iter()
            .any(|line| line.contains("(metadata unreadable)")),
        "a project whose range was read is never said to be unreadable: {drawn:?}"
    );
    Ok(())
}

#[test]
fn a_project_whose_metadata_could_not_be_read_names_that_instead_of_a_range() -> Checked {
    let screen = RecordedScreen::new();
    let keys = [Key::ArrowRight, Key::Enter];
    let chosen = prompt(ScriptedKeys::pressing(&keys), &screen)
        .select_open(&heading(), &labels(), &[None, Some(2), Some(2)])
        .required_because("an unreadable project can still be confirmed")?;

    assert_eq!(chosen, (0, 0), "the index does not move without a range");
    let drawn = screen.drawn();
    assert!(
        drawn
            .iter()
            .any(|line| line.contains("Worktree index: 0 (metadata unreadable)")),
        "the reason is named rather than a range: {drawn:?}"
    );
    assert!(
        !drawn.iter().any(|line| line.contains("(0-")),
        "no range is made up for a project that was not read: {drawn:?}"
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
        .exact(&msg!("destroy-confirm-prompt", project = "owner/repo"))
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
        .exact(&msg!("destroy-confirm-prompt", project = "owner/repo"))
        .required_because("the name is typed in full")?;

    assert_eq!(typed, "owner-repo");
    assert_eq!(
        screen.lines(),
        vec![
            "Type owner/repo to confirm the deletion".to_string(),
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
        .exact(&msg!("destroy-confirm-prompt", project = "owner/repo"))
        .required_because("the answer is confirmed")?;

    assert_eq!(typed, "a");
    Ok(())
}

#[test]
fn a_cancelled_input_returns_nothing_that_could_be_taken_as_an_answer() -> Checked {
    for key in [Key::Escape, Key::CtrlC] {
        let screen = RecordedScreen::new();
        let error = prompt(ScriptedKeys::pressing(std::slice::from_ref(&key)), &screen)
            .exact(&msg!("destroy-confirm-prompt", project = "owner/repo"))
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
        .exact(&msg!("destroy-confirm-prompt", project = "owner/repo"))
        .required_because("the answer is confirmed")?;

    assert_eq!(
        screen.lines().first().map(String::as_str),
        Some("削除を確認するため owner/repo と入力してください")
    );
    Ok(())
}

#[test]
fn an_open_prompt_with_no_projects_is_unresolved_rather_than_an_empty_prompt() -> Checked {
    let screen = RecordedScreen::new();
    let error = prompt(ScriptedKeys::confirming(), &screen)
        .select_open(&heading(), &[], &[])
        .refused_because("there is no project to choose")?;

    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    assert_ne!(error.exit_code(), ExitCode::Canceled);
    assert!(screen.drawn().is_empty(), "nothing is drawn");
    Ok(())
}

#[test]
fn an_open_prompt_that_cannot_be_drawn_or_read_stops_without_a_choice() -> Checked {
    for (keys, screen, reason) in [
        (
            ScriptedKeys::confirming(),
            RecordedScreen::failing(std::io::ErrorKind::BrokenPipe),
            "a prompt that cannot be drawn cannot be answered",
        ),
        (
            ScriptedKeys::failing(std::io::ErrorKind::BrokenPipe),
            RecordedScreen::new(),
            "a prompt that cannot be read cannot be answered",
        ),
    ] {
        let error = prompt(keys, &screen)
            .select_open(&heading(), &labels(), &[Some(2); 3])
            .refused_because(reason)?;

        assert_eq!(
            error.first_id(),
            Some(ErrorId::PromptUnreadable),
            "{reason}"
        );
        // 描いた一覧は下ろし、確定の一行も残さない。
        assert!(screen.lines().is_empty(), "{reason}: {:?}", screen.lines());
        assert!(
            screen.cursor_is_visible(),
            "{reason}: the cursor is handed back"
        );
    }
    Ok(())
}

#[test]
fn an_input_stops_at_the_first_write_the_screen_refuses() -> Checked {
    let heading = "Enter the name this project's commits are made under".to_string();
    // (打鍵, 候補, 受け付ける操作の数, 書けなくなった操作)。
    let cases: [(&[Key], &str, usize, &str); 5] = [
        (&[Key::Enter], "", 0, "drawing the heading"),
        (&[Key::Enter], "Host User", 1, "placing the candidate"),
        (
            &[Key::Char('a'), Key::Enter],
            "",
            1,
            "echoing a typed character",
        ),
        (
            &[Key::Char('a'), Key::Backspace, Key::Enter],
            "",
            2,
            "rubbing out a character",
        ),
        (&[Key::Enter], "", 1, "ending the line"),
    ];
    for (keys, candidate, allowed, failed) in cases {
        let screen = RecordedScreen::failing_after(allowed, std::io::ErrorKind::BrokenPipe);
        let error = prompt(ScriptedKeys::pressing(keys), &screen)
            .input(&msg!("prompt-git-user-name"), candidate)
            .refused_because(&format!(
                "a screen that fails while {failed} stops the input"
            ))?;

        assert_eq!(
            error.first_id(),
            Some(ErrorId::PromptUnreadable),
            "{failed}"
        );
        assert_ne!(error.exit_code(), ExitCode::Canceled, "{failed}");
        // 書けなかった操作より後は、何も描いたものとして数えない。
        let expected = if allowed == 0 {
            Vec::new()
        } else {
            vec![heading.clone()]
        };
        assert_eq!(screen.drawn(), expected, "{failed}");
    }
    Ok(())
}

#[test]
fn a_heading_that_cannot_be_formatted_names_the_failure_and_still_asks() -> Checked {
    // 利用者向けの文字列を作れなくても、promptを止めず内部異常の文字列をそのまま見せる。
    let screen = RecordedScreen::new();
    let chosen = prompt(ScriptedKeys::confirming(), &screen)
        .select_one(&msg!("no-such-prompt-heading"), &labels())
        .required_because("the candidates can still be chosen")?;

    assert_eq!(chosen, 0);
    let heading = screen
        .drawn()
        .first()
        .cloned()
        .required_because("the heading line is drawn")?;
    assert!(
        heading.starts_with("message-format-failed: message-id=no-such-prompt-heading"),
        "{heading}"
    );
    Ok(())
}
