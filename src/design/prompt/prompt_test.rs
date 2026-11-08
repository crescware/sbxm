use crate::design::policy::StreamPolicy;
use crate::diagnostics::ErrorId;
use crate::i18n::{Catalog, Locale};
use crate::msg;

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
    Selection::new(&labels(), false, true, Filter::None)
}

fn multi() -> Selection {
    Selection::new(&labels(), true, true, Filter::None)
}

/// 文字で絞り込める選択。
fn typed(multi: bool) -> Selection {
    Selection::new(&labels(), multi, true, Filter::Typed)
}

fn type_text(selection: &mut Selection, text: &str) {
    for character in text.chars() {
        assert_eq!(
            selection.apply(Action::Type(character)),
            Transition::Continue
        );
    }
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
            selection.apply(action_for(key)),
            Transition::Canceled,
            "{key:?}"
        );
    }
}

#[test]
fn keys_that_have_no_meaning_leave_the_state_alone() {
    let mut selection = multi();
    let before = selection.clone();
    // 絞り込めない一覧では、文字とBackspaceも意味を持たない。
    for key in [
        Key::Tab,
        Key::ArrowLeft,
        Key::Char('x'),
        Key::Backspace,
        Key::Unknown,
    ] {
        assert_eq!(selection.apply(action_for(key)), Transition::Continue);
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
fn an_empty_selection_has_nowhere_to_move_and_confirms_nothing() {
    // `PromptUi`は候補ゼロで開かない。`Selection`は端末を持たない状態機械として単体で
    // 成立するため、候補ゼロもこの型自身が受け止める。
    let mut selection = Selection::new(&[], true, true, Filter::None);
    assert_eq!(selection.apply(Action::Next), Transition::Continue);
    assert_eq!(selection.apply(Action::Previous), Transition::Continue);
    assert_eq!(selection.current(), 0, "there is no row to move onto");
    assert_eq!(selection.selected_count(), 0);
    assert_eq!(
        selection.apply(Action::Confirm),
        Transition::Continue,
        "an empty confirmation is refused rather than answered with nothing"
    );
    assert!(selection.warns_about_empty());
}

#[test]
fn color_marks_focus_and_selection_without_changing_a_single_word() {
    let mut selection = multi();
    selection.apply(Action::Next);
    selection.apply(Action::Toggle);
    let plain = frame(&selection, StreamPolicy::plain());
    let colored = frame(&selection, StreamPolicy::colored());

    assert_eq!(colored.len(), plain.len(), "color adds no line");
    let stripped: Vec<String> = colored.iter().map(|line| strip_ansi(line)).collect();
    assert_eq!(stripped, plain, "color adds decoration, never text");

    // focusのある行はcyanのbold、選択済みのcheckboxはgreenで示す。glyphと色の両方を出し、
    // どちらか一方だけでも状態が読めるようにする。
    let rows = &colored[colored.len() - 3..];
    assert!(
        rows[1].starts_with("\u{1b}[36m\u{1b}[1m\u{203a}"),
        "{rows:?}"
    );
    assert!(rows[1].contains("\u{1b}[32m[x]"), "{rows:?}");
    assert!(
        !rows[0].contains('\u{1b}') && !rows[2].contains('\u{1b}'),
        "rows that are neither focused nor selected stay undecorated: {rows:?}"
    );
}

/// ANSI sequenceを取り除く。色の有無で文字が変わらないことの比較にだけ使う。
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            out.push(character);
            continue;
        }
        for inner in characters.by_ref() {
            if inner.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

#[test]
fn the_operation_guide_pairs_every_key_with_what_it_does() {
    let single = painter(StreamPolicy::plain()).keys(false, Filter::None);
    assert!(single.contains("Enter Confirm"), "{single}");
    assert!(single.contains("Esc Cancel"), "{single}");
    assert!(
        !single.contains("Space"),
        "a single selection has no toggle"
    );
    assert!(
        !single.contains("Type"),
        "a list that cannot be narrowed does not invite typing"
    );

    let multi = painter(StreamPolicy::plain()).keys(true, Filter::None);
    assert!(multi.contains("Space Toggle"), "{multi}");

    let typed = painter(StreamPolicy::plain()).keys(true, Filter::Typed);
    assert!(
        typed.contains("Space Toggle   Type Filter   Enter Confirm"),
        "{typed}"
    );
}

#[test]
fn the_arrow_keys_fall_back_to_ascii() {
    let policy = StreamPolicy {
        characters: CharacterSet::Ascii,
        ..StreamPolicy::plain()
    };
    let keys = painter(policy).keys(false, Filter::None);
    assert!(keys.starts_with("^/v Move"), "{keys}");
}

#[test]
fn a_single_candidate_still_shows_its_state() -> Checked {
    let labels = ["owner/alpha".to_string()];
    let selection = Selection::new(&labels, true, true, Filter::None);
    let drawn = painter(StreamPolicy::plain()).frame(
        &msg!("select-stop-heading"),
        &labels,
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
fn a_height_that_cannot_be_read_is_not_a_limit_on_the_list() {
    assert_eq!(
        viewport(None, Filter::None),
        None,
        "an unknown height shows every row"
    );
    // heading、操作説明、選択数、空行、結果の一行ぶんを残す。
    assert_eq!(viewport(Some(24), Filter::None), Some(18));
    assert_eq!(viewport(Some(7), Filter::None), Some(1));
    assert_eq!(
        viewport(Some(2), Filter::None),
        Some(1),
        "a screen too short for the frame still shows one candidate"
    );
    // 絞り込める一覧は、入力の一行も残す。
    assert_eq!(viewport(Some(24), Filter::Typed), Some(17));
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

#[test]
fn the_open_frame_reserves_exactly_the_rows_the_viewport_subtracts() {
    let labels = labels();
    let selection = OpenSelection::new(&labels, &[Some(4); 3]);
    let drawn = painter(StreamPolicy::plain()).open_frame(
        &msg!("select-open-heading"),
        &labels,
        &selection,
        None,
    );

    assert_eq!(
        drawn.len() - labels.len(),
        OPEN_FRAME_FIXED_ROWS,
        "the list height is computed from this count: {drawn:?}"
    );
}

#[test]
fn the_filter_line_invites_typing_until_something_is_typed() {
    let mut selection = typed(false);
    let drawn = frame(&selection, StreamPolicy::plain());
    assert_eq!(
        drawn[2],
        "  \u{2191}/\u{2193} Move   Type Filter   Enter Confirm   Esc Cancel"
    );
    assert_eq!(drawn[3], "  Filter: type to narrow the list");
    assert_eq!(
        drawn.len(),
        8,
        "heading, blank, keys, filter, blank and 3 rows: {drawn:?}"
    );

    type_text(&mut selection, "br");
    let drawn = frame(&selection, StreamPolicy::plain());
    assert_eq!(
        drawn[3], "  Filter: br  1/3",
        "what was typed and how many are left"
    );
    assert_eq!(
        &drawn[drawn.len() - 1..],
        ["\u{203a} owner/bravo  (current)"],
        "only the matching candidate is drawn, and it takes the focus: {drawn:?}"
    );
    assert_eq!(selection.apply(Action::Confirm), Transition::Done(vec![1]));
}

#[test]
fn the_focus_stays_on_its_candidate_while_it_matches_and_moves_to_the_first_match_otherwise() {
    let mut selection = typed(false);
    selection.apply(Action::Next);
    type_text(&mut selection, "a");
    assert_eq!(selection.visible(), vec![0, 1, 2], "every label has an a");
    assert_eq!(
        selection.current(),
        1,
        "the focus does not jump while bravo still matches"
    );

    type_text(&mut selection, "l");
    assert_eq!(selection.visible(), vec![0]);
    assert_eq!(
        selection.current(),
        0,
        "bravo is hidden, so alpha takes the focus"
    );

    assert_eq!(selection.apply(Action::Erase), Transition::Continue);
    assert_eq!(
        selection.current(),
        0,
        "alpha is still visible and keeps the focus"
    );
    assert_eq!(selection.apply(Action::Next), Transition::Continue);
    assert_eq!(selection.current(), 1, "the arrows move among the matches");
}

#[test]
fn a_filter_that_matches_nothing_warns_instead_of_confirming_a_hidden_candidate() {
    let mut selection = typed(true);
    selection.apply(Action::Toggle);
    type_text(&mut selection, "zz");
    assert!(selection.visible().is_empty());
    assert_eq!(
        selection.apply(Action::Toggle),
        Transition::Continue,
        "a hidden candidate cannot be toggled"
    );
    assert_eq!(
        selection.selected_count(),
        1,
        "the earlier selection is kept"
    );
    assert_eq!(selection.apply(Action::Confirm), Transition::Continue);
    assert!(selection.warns_about_no_match());

    let drawn = frame(&selection, StreamPolicy::plain());
    assert_eq!(
        drawn.last().map(String::as_str),
        Some(""),
        "no candidate row is drawn: {drawn:?}"
    );
    assert!(
        drawn.contains(
            &"! Warning: Nothing matches zz. Press Backspace to erase it, or Esc to cancel."
                .to_string()
        ),
        "{drawn:?}"
    );

    assert_eq!(selection.apply(Action::Erase), Transition::Continue);
    assert!(
        !selection.warns_about_no_match(),
        "erasing takes the warning back down"
    );
    assert_eq!(selection.apply(Action::Erase), Transition::Continue);
    assert_eq!(selection.apply(Action::Confirm), Transition::Done(vec![0]));
}

#[test]
fn checked_candidates_hidden_by_the_filter_are_counted_and_still_confirmed() {
    let mut selection = typed(true);
    selection.apply(Action::Toggle);
    type_text(&mut selection, "br");
    assert_eq!(selection.selected_count(), 1);
    assert_eq!(selection.hidden_checked_count(), 1);

    let drawn = frame(&selection, StreamPolicy::plain());
    assert!(
        drawn.contains(&"  Selected: 1 (1 hidden by the filter)".to_string()),
        "a selection out of view is still announced: {drawn:?}"
    );
    assert_eq!(
        selection.apply(Action::Confirm),
        Transition::Done(vec![0]),
        "the hidden selection is what gets confirmed"
    );
}

#[test]
fn the_matched_letters_are_underlined_and_nothing_else_changes() {
    let mut selection = typed(true);
    selection.apply(Action::Next);
    type_text(&mut selection, "r/");
    let plain = frame(&selection, StreamPolicy::plain());
    let colored = frame(&selection, StreamPolicy::colored());

    let stripped: Vec<String> = colored.iter().map(|line| strip_ansi(line)).collect();
    assert_eq!(stripped, plain, "underline adds decoration, never text");

    let rows = &colored[colored.len() - 3..];
    assert!(
        rows[0].contains("[ ] owne\u{1b}[4mr/\u{1b}[0malpha"),
        "a row without focus underlines only the match: {rows:?}"
    );
    assert!(
        rows[1].contains("\u{1b}[36m\u{1b}[1m\u{1b}[4mr/\u{1b}[0m"),
        "the focused row keeps its weight and color under the line: {rows:?}"
    );
    assert!(
        !plain.iter().any(|line| line.contains('\u{1b}')),
        "without color the filter line is what says what matched: {plain:?}"
    );
}

#[test]
fn a_narrow_terminal_underlines_only_the_part_of_the_match_that_is_shown() {
    let policy = StreamPolicy {
        width: Some(20),
        ..StreamPolicy::colored()
    };
    let mut selection = typed(false);
    type_text(&mut selection, "r/");
    let drawn = frame(&selection, policy);
    let rows = &drawn[drawn.len() - 3..];

    // 現在位置の行は`(current)`のぶん狭く、一致した部分が省略記号へ落ちる。
    assert!(
        rows[0].contains("owne\u{1b}[0m\u{1b}[36m\u{1b}[1m...\u{1b}[0m"),
        "the cut-away match is not underlined and the ellipsis stays plain: {rows:?}"
    );
    assert!(!rows[0].contains("\u{1b}[4m"), "{rows:?}");
    assert!(
        rows[1].contains("owne\u{1b}[4mr/\u{1b}[0mbravo"),
        "a row that fits underlines its match: {rows:?}"
    );
}

#[test]
fn the_open_frame_draws_the_filter_and_warns_when_nothing_matches() {
    let labels = labels();
    let mut selection = OpenSelection::new(&labels, &[Some(4); 3]);
    for character in "zz".chars() {
        selection.apply(Action::Type(character));
    }
    selection.apply(Action::Confirm);
    let drawn = painter(StreamPolicy::plain()).open_frame(
        &msg!("select-open-heading"),
        &labels,
        &selection,
        None,
    );

    assert_eq!(
        drawn[2],
        "  \u{2191}/\u{2193} Move project   \u{2190}/\u{2192} Change index   Type Filter   Enter Confirm   Esc Cancel"
    );
    assert_eq!(drawn[4], "  Filter: zz  0/3");
    assert_eq!(drawn[5], "  Worktree index: 0 (0-4)");
    assert_eq!(
        drawn[6],
        "! Warning: Nothing matches zz. Press Backspace to erase it, or Esc to cancel."
    );
    assert_eq!(
        drawn.len(),
        OPEN_FRAME_FIXED_ROWS + 1,
        "no project row: {drawn:?}"
    );
}
