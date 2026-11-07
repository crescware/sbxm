use super::super::Filter;
use super::Narrowing;

fn labels() -> Vec<String> {
    ["Owner/Alpha", "owner/bravo", "other/alphabet"]
        .iter()
        .map(|label| (*label).to_string())
        .collect()
}

fn typed(query: &str) -> Narrowing {
    let mut narrowing = Narrowing::new(&labels(), Filter::Typed);
    for character in query.chars() {
        assert!(narrowing.type_char(character), "{character:?} is accepted");
    }
    narrowing
}

#[test]
fn an_empty_query_shows_every_candidate_and_underlines_nothing() {
    let narrowing = typed("");
    assert!(!narrowing.has_query());
    assert_eq!(narrowing.matches(), vec![0, 1, 2]);
    assert_eq!(narrowing.total(), 3);
    for index in 0..3 {
        assert!(narrowing.is_match(index), "{index}");
        assert_eq!(narrowing.match_range(index), None, "{index}");
    }
}

#[test]
fn a_query_is_a_contiguous_substring_matched_without_regard_to_case() {
    let narrowing = typed("ALP");
    assert_eq!(
        narrowing.query(),
        "ALP",
        "the query keeps the letters as typed"
    );
    assert_eq!(
        narrowing.matches(),
        vec![0, 2],
        "the order of the candidates is kept"
    );
    assert_eq!(narrowing.match_range(0), Some(6..9));
    assert_eq!(narrowing.match_range(2), Some(6..9));
    assert_eq!(narrowing.match_range(1), None);
    assert!(!narrowing.is_match(1));

    // 飛び石の一致は採らない。
    assert!(typed("oa").matches().is_empty());
    // 候補より長い文字列は何にも一致しない。
    assert!(typed("owner/alphabetical").matches().is_empty());
    // 候補の外のindexは一致でも範囲でもない。
    assert!(!narrowing.is_match(3));
    assert_eq!(narrowing.match_range(3), None);
}

#[test]
fn only_the_first_occurrence_is_underlined() {
    let mut narrowing = Narrowing::new(&["abab".to_string()], Filter::Typed);
    narrowing.type_char('a');
    narrowing.type_char('b');
    assert_eq!(narrowing.match_range(0), Some(0..2));
}

#[test]
fn the_range_stays_on_character_boundaries_of_the_original_label() {
    let mut narrowing =
        Narrowing::new(&["案件/sbxm".to_string(), "ΣΒΧ".to_string()], Filter::Typed);
    narrowing.type_char('S');
    narrowing.type_char('b');
    assert_eq!(
        narrowing.match_range(0),
        Some(7..9),
        "wide characters before the match take their full byte length"
    );
    narrowing.erase();
    narrowing.erase();
    narrowing.type_char('σ');
    narrowing.type_char('β');
    assert_eq!(
        narrowing.match_range(1),
        Some(0..4),
        "letters outside ASCII are folded the same way"
    );
}

#[test]
fn the_cursor_stays_on_a_candidate_that_survives_and_moves_to_the_first_match_otherwise() {
    let narrowing = typed("alp");
    assert_eq!(
        narrowing.anchored(2),
        2,
        "a visible candidate keeps the focus"
    );
    assert_eq!(
        narrowing.anchored(1),
        0,
        "a hidden candidate gives the focus to the first match"
    );
}

#[test]
fn a_query_that_matches_nothing_remembers_the_candidate_until_it_is_erased() {
    let mut narrowing = typed("zzz");
    assert!(narrowing.matches().is_empty());
    assert_eq!(
        narrowing.anchored(1),
        1,
        "nothing to move onto, so nothing moves"
    );
    assert_eq!(narrowing.next(1), 1);
    assert_eq!(narrowing.previous(1), 1);

    for _ in 0..3 {
        assert!(narrowing.erase());
    }
    assert!(!narrowing.erase(), "there is nothing left to erase");
    assert_eq!(
        narrowing.anchored(1),
        1,
        "the remembered candidate is back in view"
    );
}

#[test]
fn moving_cycles_through_the_matches_only() {
    let narrowing = typed("alp");
    assert_eq!(
        narrowing.next(0),
        2,
        "the hidden candidate between them is skipped"
    );
    assert_eq!(narrowing.next(2), 0, "the end wraps to the start");
    assert_eq!(narrowing.previous(0), 2, "the start wraps to the end");
    assert_eq!(narrowing.previous(2), 0);
    assert_eq!(
        narrowing.next(1),
        0,
        "a hidden current position moves onto the first match"
    );
}

#[test]
fn a_prompt_that_cannot_be_narrowed_takes_no_characters() {
    let mut narrowing = Narrowing::new(&labels(), Filter::None);
    assert_eq!(narrowing.filter(), Filter::None);
    assert!(!narrowing.is_typed());
    assert!(!narrowing.type_char('a'), "the character is refused");
    assert!(!narrowing.has_query());
    assert_eq!(narrowing.matches(), vec![0, 1, 2]);
    assert!(!narrowing.erase());
}

#[test]
fn a_narrowing_without_candidates_has_nothing_to_move_onto() {
    let mut narrowing = Narrowing::new(&[], Filter::Typed);
    assert_eq!(narrowing.next(0), 0);
    assert_eq!(narrowing.previous(0), 0);
    assert_eq!(narrowing.anchored(0), 0);
    narrowing.type_char('a');
    assert!(narrowing.matches().is_empty());
}
