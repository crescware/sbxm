use crate::design::policy::StreamPolicy;
use crate::diagnostics::ErrorId;
use crate::i18n::{Catalog, Locale};
use crate::msg;
use console::Key;

use crate::testing::outcome::{Checked, Required};

use super::*;

use crate::design::policy::CharacterSet;

fn labels() -> Vec<String> {
    ["owner/alpha", "owner/bravo", "owner/charlie"]
        .iter()
        .map(|label| (*label).to_string())
        .collect()
}

fn painter(policy: StreamPolicy) -> Painter {
    Painter {
        catalog: Catalog::new(Locale::En),
        policy,
    }
}

fn frame(selection: &Selection, policy: StreamPolicy) -> Vec<String> {
    painter(policy).frame(&msg!("select-open-heading"), &labels(), selection, None)
}

fn single() -> Selection {
    Selection::new(3, false, true)
}

fn multi() -> Selection {
    Selection::new(3, true, true)
}

#[test]
fn nothing_is_selected_before_enter_is_pressed() {
    let selection = multi();
    assert_eq!(selection.selected_count(), 0);
    assert!(!selection.is_checked(0), "the first row is not implicit");
}

#[test]
fn moving_wraps_at_both_ends_without_reordering_the_candidates() {
    let mut selection = single();
    selection.apply(Action::Previous);
    assert_eq!(selection.current(), 2, "moving up from the top wraps");
    selection.apply(Action::Next);
    assert_eq!(selection.current(), 0, "moving down from the bottom wraps");
}

#[test]
fn focus_and_selection_change_independently() {
    let mut selection = multi();
    selection.apply(Action::Next);
    selection.apply(Action::Toggle);
    assert_eq!(selection.current(), 1);
    assert!(selection.is_checked(1));

    selection.apply(Action::Next);
    assert_eq!(
        selection.current(),
        2,
        "moving does not clear the selection"
    );
    assert!(selection.is_checked(1));
    assert!(!selection.is_checked(2));
}

#[test]
fn the_current_row_can_also_be_the_selected_one() {
    let mut selection = multi();
    selection.apply(Action::Toggle);
    let drawn = frame(&selection, StreamPolicy::plain());
    let drawn = &drawn[drawn.len() - 3];
    assert!(drawn.starts_with('\u{203a}'), "focus is shown: {drawn:?}");
    assert!(drawn.contains("[x]"), "selection is shown: {drawn:?}");
    assert!(drawn.contains("(current)"), "{drawn:?}");
}

#[test]
fn the_selected_count_tracks_every_toggle_including_zero() {
    let mut selection = multi();
    assert!(frame(&selection, StreamPolicy::plain()).contains(&"  Selected: 0".to_string()));

    selection.apply(Action::Toggle);
    selection.apply(Action::Next);
    selection.apply(Action::Toggle);
    assert_eq!(selection.selected_count(), 2);
    assert!(frame(&selection, StreamPolicy::plain()).contains(&"  Selected: 2".to_string()));

    selection.apply(Action::Toggle);
    assert_eq!(selection.selected_count(), 1);
}

#[test]
fn a_single_selection_ignores_the_toggle_key() {
    let mut selection = single();
    selection.apply(Action::Toggle);
    assert_eq!(selection.selected_count(), 0);
}

#[test]
fn confirming_a_single_selection_returns_the_focused_row() {
    let mut selection = single();
    selection.apply(Action::Next);
    assert_eq!(selection.apply(Action::Confirm), Transition::Done(vec![1]));
}

#[test]
fn confirming_nothing_warns_and_keeps_the_position() {
    let mut selection = multi();
    selection.apply(Action::Next);
    assert_eq!(selection.apply(Action::Confirm), Transition::Continue);
    assert!(selection.warns_about_empty());
    assert_eq!(selection.current(), 1, "the prompt does not jump back");

    let drawn = frame(&selection, StreamPolicy::plain());
    assert!(
        drawn.iter().any(|line| line.starts_with("! Warning: ")),
        "{drawn:?}"
    );
}

#[test]
fn selecting_something_takes_the_warning_back_down() {
    let mut selection = multi();
    selection.apply(Action::Confirm);
    assert!(selection.warns_about_empty());
    selection.apply(Action::Toggle);
    assert!(!selection.warns_about_empty());
}

#[test]
fn a_multi_selection_confirms_every_checked_row_in_order() {
    let mut selection = multi();
    selection.apply(Action::Next);
    selection.apply(Action::Next);
    selection.apply(Action::Toggle);
    selection.apply(Action::Previous);
    selection.apply(Action::Previous);
    selection.apply(Action::Toggle);
    assert_eq!(
        selection.apply(Action::Confirm),
        Transition::Done(vec![0, 2])
    );
}

#[test]
fn escape_and_ctrl_c_change_nothing() {
    for key in [Key::Escape, Key::CtrlC] {
        let mut selection = multi();
        selection.apply(Action::Toggle);
        assert_eq!(
            selection.apply(action_for(&key)),
            Transition::Canceled,
            "{key:?}"
        );
    }
}

#[test]
fn keys_that_have_no_meaning_leave_the_state_alone() {
    let mut selection = multi();
    let before = selection.clone();
    for key in [Key::Tab, Key::ArrowLeft, Key::Char('x'), Key::Unknown] {
        assert_eq!(selection.apply(action_for(&key)), Transition::Continue);
    }
    assert_eq!(selection, before);
}

#[test]
fn every_state_stays_readable_without_color() {
    let mut selection = multi();
    selection.apply(Action::Next);
    selection.apply(Action::Toggle);
    let drawn = frame(&selection, StreamPolicy::plain());

    let rows = &drawn[drawn.len() - 3..];
    assert_eq!(rows[0], "  [ ] owner/alpha", "{rows:?}");
    assert!(rows[1].starts_with("\u{203a} [x] owner/bravo"), "{rows:?}");
    assert!(rows[1].ends_with("(current)"), "{rows:?}");
    assert_eq!(rows[2], "  [ ] owner/charlie", "{rows:?}");
}

#[test]
fn the_operation_guide_pairs_every_key_with_what_it_does() {
    let single = painter(StreamPolicy::plain()).keys(false);
    assert!(single.contains("Enter Confirm"), "{single}");
    assert!(single.contains("Esc Cancel"), "{single}");
    assert!(
        !single.contains("Space"),
        "a single selection has no toggle"
    );

    let multi = painter(StreamPolicy::plain()).keys(true);
    assert!(multi.contains("Space Toggle"), "{multi}");
}

#[test]
fn the_arrow_keys_fall_back_to_ascii() {
    let policy = StreamPolicy {
        characters: CharacterSet::Ascii,
        ..StreamPolicy::plain()
    };
    let keys = painter(policy).keys(false);
    assert!(keys.starts_with("^/v Move"), "{keys}");
}

#[test]
fn a_single_candidate_still_shows_its_state() -> Checked {
    let selection = Selection::new(1, true, true);
    let drawn = painter(StreamPolicy::plain()).frame(
        &msg!("select-stop-heading"),
        &["owner/alpha".to_string()],
        &selection,
        None,
    );
    let row = drawn.last().required_because("the only candidate")?;
    assert!(row.starts_with("\u{203a} [ ] owner/alpha"), "{row:?}");
    assert!(row.contains("(current)"), "{row:?}");
    Ok(())
}

#[test]
fn a_narrow_terminal_shortens_the_label_and_keeps_the_markers() {
    let policy = StreamPolicy {
        width: Some(24),
        ..StreamPolicy::plain()
    };
    let selection = multi();
    let drawn = frame(&selection, policy);
    let row = &drawn[drawn.len() - 3];
    assert!(row.starts_with("\u{203a} [ ] "), "{row:?}");
    assert!(row.contains("..."), "the label is what gets cut: {row:?}");
    assert!(row.contains("(current)"), "{row:?}");
}

#[test]
fn the_viewport_keeps_the_current_row_visible() {
    assert_eq!(window(10, 0, Some(3)), 0..3);
    assert_eq!(
        window(10, 5, Some(3)),
        4..7,
        "the window follows the cursor"
    );
    assert_eq!(window(10, 9, Some(3)), 7..10, "it stops at the end");
    assert_eq!(window(2, 1, Some(5)), 0..2, "a short list is not padded");
    assert_eq!(window(10, 4, None), 0..10, "an unknown height shows all");
}

#[test]
fn an_interrupted_read_is_a_cancel_and_any_other_failure_is_reported() {
    let canceled = unreadable(&std::io::Error::from(std::io::ErrorKind::Interrupted));
    assert_eq!(canceled.exit_code(), crate::diagnostics::ExitCode::Canceled);

    let broken = unreadable(&std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    assert_eq!(broken.first_id(), Some(ErrorId::PromptUnreadable));
    assert_ne!(broken.exit_code(), crate::diagnostics::ExitCode::Canceled);
}

#[test]
fn the_confirmed_value_is_left_as_its_own_line() {
    let line = painter(StreamPolicy::plain()).selected("owner/bravo");
    assert_eq!(line, "\u{2713} Selected owner/bravo");
}
