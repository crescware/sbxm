use super::open_viewport;

#[test]
fn the_open_list_gives_back_one_row_less_than_the_plain_list() {
    assert_eq!(
        open_viewport(Some(40)),
        Some(33),
        "the worktree index line takes a row that the plain prompt does not have"
    );
}

#[test]
fn a_terminal_too_short_for_the_open_prompt_still_shows_one_candidate() {
    assert_eq!(
        open_viewport(Some(4)),
        Some(1),
        "a list of no rows would hide the selection entirely"
    );
}

#[test]
fn an_open_prompt_on_an_unknown_height_does_not_limit_the_list() {
    assert_eq!(
        open_viewport(None),
        None,
        "an unknown height is not a height of zero"
    );
}
