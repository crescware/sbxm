use super::super::Filter;
use super::viewport;

#[test]
fn the_list_gives_back_the_rows_that_the_rest_of_the_prompt_needs() {
    assert_eq!(
        viewport(Some(40), Filter::None),
        Some(34),
        "heading, keys, count, blank and result keep their rows"
    );
    assert_eq!(
        viewport(Some(40), Filter::Typed),
        Some(33),
        "a list that can be narrowed also keeps the row the query is typed on"
    );
}

#[test]
fn a_terminal_too_short_for_the_prompt_still_shows_one_candidate() {
    for filter in [Filter::None, Filter::Typed] {
        assert_eq!(
            viewport(Some(4), filter),
            Some(1),
            "a list of no rows would hide the selection entirely: {filter:?}"
        );
    }
}

#[test]
fn a_stream_that_is_not_a_terminal_does_not_limit_the_list() {
    assert_eq!(
        viewport(None, Filter::Typed),
        None,
        "an unknown height is not a height of zero"
    );
}
