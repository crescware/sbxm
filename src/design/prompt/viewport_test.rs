use super::viewport;

#[test]
fn the_list_gives_back_the_rows_that_the_rest_of_the_prompt_needs() {
    assert_eq!(
        viewport(Some(40)),
        Some(34),
        "heading, keys, count, blank and result keep their rows"
    );
}

#[test]
fn a_terminal_too_short_for_the_prompt_still_shows_one_candidate() {
    assert_eq!(
        viewport(Some(4)),
        Some(1),
        "a list of no rows would hide the selection entirely"
    );
}

#[test]
fn a_stream_that_is_not_a_terminal_does_not_limit_the_list() {
    assert_eq!(
        viewport(None),
        None,
        "an unknown height is not a height of zero"
    );
}
